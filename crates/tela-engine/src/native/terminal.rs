//! Bounded, in-process virtual terminal surfaces. No parsed sequence is ever
//! written to the host terminal; the renderer reads the vt100 grid directly.
use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rquickjs::Ctx;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const MAX_TERMINALS: usize = 16;
const MAX_COLS: u16 = 320;
const MAX_ROWS: u16 = 160;
const MAX_WRITE_BYTES: usize = 1024 * 1024;
const MAX_BASE64_BYTES: usize = ((MAX_WRITE_BYTES + 2) / 3) * 4;
const SCROLLBACK_ROWS: usize = 2_000;

pub type TerminalHandles = Arc<Mutex<TerminalStore>>;

pub struct TerminalStore {
    next_id: u64,
    surfaces: HashMap<u64, TerminalSurface>,
}

struct TerminalSurface {
    parser: vt100::Parser,
    cols: u16,
    rows: u16,
}

#[derive(Serialize)]
pub struct TerminalModes {
    #[serde(rename = "applicationCursor")]
    application_cursor: bool,
    #[serde(rename = "applicationKeypad")]
    application_keypad: bool,
    #[serde(rename = "bracketedPaste")]
    bracketed_paste: bool,
}

impl TerminalStore {
    fn new() -> Self {
        Self {
            next_id: 1,
            surfaces: HashMap::new(),
        }
    }

    pub fn create(&mut self, cols: u16, rows: u16) -> Result<u64> {
        validate_size(cols, rows)?;
        if self.surfaces.len() >= MAX_TERMINALS {
            bail!("terminal limit ({MAX_TERMINALS}) reached");
        }
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("terminal id exhausted"))?;
        self.surfaces.insert(
            id,
            TerminalSurface {
                parser: vt100::Parser::new(rows, cols, SCROLLBACK_ROWS),
                cols,
                rows,
            },
        );
        Ok(id)
    }

    pub fn write(&mut self, id: u64, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_WRITE_BYTES {
            bail!("terminal write exceeds {MAX_WRITE_BYTES} bytes");
        }
        self.surfaces
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown terminal {id}"))?
            .parser
            .process(bytes);
        Ok(())
    }

    pub fn resize(&mut self, id: u64, cols: u16, rows: u16) -> Result<()> {
        validate_size(cols, rows)?;
        let surface = self
            .surfaces
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown terminal {id}"))?;
        surface.parser.set_size(rows, cols);
        surface.cols = cols;
        surface.rows = rows;
        Ok(())
    }

    pub fn dispose(&mut self, id: u64) -> bool {
        self.surfaces.remove(&id).is_some()
    }
    pub fn reset(&mut self, id: u64) -> Result<()> {
        // Recreate the parser so partially parsed input cannot survive a reset.
        let surface = self
            .surfaces
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown terminal {id}"))?;
        surface.parser = vt100::Parser::new(surface.rows, surface.cols, SCROLLBACK_ROWS);
        Ok(())
    }

    pub fn modes(&self, id: u64) -> Result<TerminalModes> {
        let screen = self
            .surfaces
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown terminal {id}"))?
            .parser
            .screen();
        Ok(TerminalModes {
            application_cursor: screen.application_cursor(),
            application_keypad: screen.application_keypad(),
            bracketed_paste: screen.bracketed_paste(),
        })
    }

    pub fn get(&self, id: u64) -> Option<&vt100::Parser> {
        self.surfaces.get(&id).map(|surface| &surface.parser)
    }
}

fn decode_base64(data: &str) -> Result<Vec<u8>> {
    if data.len() > MAX_BASE64_BYTES {
        bail!("terminal base64 write exceeds {MAX_BASE64_BYTES} encoded bytes");
    }
    if data.len() % 4 != 0 {
        bail!("terminal base64 must use canonical standard padding");
    }

    let padding = data
        .as_bytes()
        .iter()
        .rev()
        .take_while(|&&b| b == b'=')
        .count();
    if padding > 2 {
        bail!("terminal base64 must use canonical standard padding");
    }
    let decoded_len = (data.len() / 4)
        .checked_mul(3)
        .and_then(|len| len.checked_sub(padding))
        .ok_or_else(|| anyhow::anyhow!("invalid terminal base64 length"))?;
    if decoded_len > MAX_WRITE_BYTES {
        bail!("terminal write exceeds {MAX_WRITE_BYTES} bytes");
    }

    // STANDARD requires canonical RFC 4648 padding and rejects non-zero trailing bits.
    let mut decoded = vec![0; decoded_len];
    let written = STANDARD
        .decode_slice(data, &mut decoded)
        .map_err(|_| anyhow::anyhow!("invalid canonical standard base64"))?;
    debug_assert_eq!(written, decoded_len);
    decoded.truncate(written);
    Ok(decoded)
}

fn validate_size(cols: u16, rows: u16) -> Result<()> {
    if cols == 0 || rows == 0 || cols > MAX_COLS || rows > MAX_ROWS {
        bail!("terminal size must be 1..={MAX_COLS} columns and 1..={MAX_ROWS} rows");
    }
    Ok(())
}

pub fn new_terminal_handles() -> TerminalHandles {
    Arc::new(Mutex::new(TerminalStore::new()))
}

const TERMINAL_BOOTSTRAP: &str = r#"
Tela.terminal = {
  create: function(options) {
    options = options || {};
    var cols = options.cols == null ? 80 : options.cols;
    var rows = options.rows == null ? 24 : options.rows;
    var id = __tela_terminal_create__(cols, rows);
    if (!id) throw new RangeError('invalid terminal size or terminal limit reached');
    return id;
  },
  write: function(id, data) {
    var ok;
    if (typeof data === 'string') ok = __tela_terminal_write_utf8__(id, data);
    else if (Array.isArray(data)) ok = __tela_terminal_write_bytes__(id, JSON.stringify(data));
    else throw new TypeError('terminal data must be a UTF-8 string or byte array');
    if (!ok) throw new RangeError('invalid terminal handle or write too large');
  },
  writeBase64: function(id, data) {
    if (typeof data !== 'string') throw new TypeError('terminal base64 data must be a string');
    if (!__tela_terminal_write_base64__(id, data)) throw new RangeError('invalid terminal handle, base64, or write too large');
  },
  modes: function(id) {
    var json = __tela_terminal_modes__(id);
    if (!json) throw new RangeError('invalid terminal handle');
    return JSON.parse(json);
  },
  resize: function(id, cols, rows) {
    if (!__tela_terminal_resize__(id, cols, rows)) throw new RangeError('invalid terminal handle or size');
  },
  dispose: function(id) { return __tela_terminal_dispose__(id); },
  reset: function(id) {
    if (!__tela_terminal_reset__(id)) throw new RangeError('invalid terminal handle');
  }
};
"#;

pub fn register_terminal(ctx: &Ctx<'_>, handles: TerminalHandles) -> Result<()> {
    let create_handles = handles.clone();
    let create = move |cols: u16, rows: u16| -> u64 {
        create_handles
            .lock()
            .ok()
            .and_then(|mut h| h.create(cols, rows).ok())
            .unwrap_or(0)
    };
    let utf8_handles = handles.clone();
    let write_utf8 = move |id: u64, text: String| -> bool {
        utf8_handles
            .lock()
            .map(|mut h| h.write(id, text.as_bytes()).is_ok())
            .unwrap_or(false)
    };
    let bytes_handles = handles.clone();
    let write_bytes = move |id: u64, bytes_json: String| -> bool {
        serde_json::from_str::<Vec<u8>>(&bytes_json)
            .ok()
            .and_then(|bytes| {
                bytes_handles
                    .lock()
                    .ok()
                    .map(|mut h| h.write(id, &bytes).is_ok())
            })
            .unwrap_or(false)
    };
    let base64_handles = handles.clone();
    let write_base64 = move |id: u64, data: String| -> bool {
        decode_base64(&data)
            .ok()
            .and_then(|bytes| {
                base64_handles
                    .lock()
                    .ok()
                    .map(|mut h| h.write(id, &bytes).is_ok())
            })
            .unwrap_or(false)
    };
    let modes_handles = handles.clone();
    let modes = move |id: u64| -> Option<String> {
        modes_handles
            .lock()
            .ok()
            .and_then(|h| h.modes(id).ok())
            .and_then(|modes| serde_json::to_string(&modes).ok())
    };
    let resize_handles = handles.clone();
    let resize = move |id: u64, cols: u16, rows: u16| -> bool {
        resize_handles
            .lock()
            .map(|mut h| h.resize(id, cols, rows).is_ok())
            .unwrap_or(false)
    };
    let dispose_handles = handles.clone();
    let dispose = move |id: u64| -> bool {
        dispose_handles
            .lock()
            .map(|mut h| h.dispose(id))
            .unwrap_or(false)
    };
    let reset = move |id: u64| -> bool {
        handles
            .lock()
            .map(|mut h| h.reset(id).is_ok())
            .unwrap_or(false)
    };
    ctx.globals().set(
        "__tela_terminal_create__",
        rquickjs::Function::new(ctx.clone(), create)?,
    )?;
    ctx.globals().set(
        "__tela_terminal_write_utf8__",
        rquickjs::Function::new(ctx.clone(), write_utf8)?,
    )?;
    ctx.globals().set(
        "__tela_terminal_write_bytes__",
        rquickjs::Function::new(ctx.clone(), write_bytes)?,
    )?;
    ctx.globals().set(
        "__tela_terminal_write_base64__",
        rquickjs::Function::new(ctx.clone(), write_base64)?,
    )?;
    ctx.globals().set(
        "__tela_terminal_modes__",
        rquickjs::Function::new(ctx.clone(), modes)?,
    )?;
    ctx.globals().set(
        "__tela_terminal_resize__",
        rquickjs::Function::new(ctx.clone(), resize)?,
    )?;
    ctx.globals().set(
        "__tela_terminal_dispose__",
        rquickjs::Function::new(ctx.clone(), dispose)?,
    )?;
    ctx.globals().set(
        "__tela_terminal_reset__",
        rquickjs::Function::new(ctx.clone(), reset)?,
    )?;
    ctx.eval::<(), _>(TERMINAL_BOOTSTRAP)
        .map_err(|e| anyhow::anyhow!("failed to register terminal bootstrap: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_is_incremental_and_resettable() {
        let mut terminals = TerminalStore::new();
        let id = terminals.create(8, 3).unwrap();
        terminals.write(id, b"hi\x1b[31m!").unwrap();
        terminals.write(id, b"\x1b[2;1Hnext").unwrap();
        let screen = terminals.get(id).unwrap().screen();
        assert_eq!(screen.cell(0, 2).unwrap().fgcolor(), vt100::Color::Idx(1));
        assert_eq!(screen.cell(1, 0).unwrap().contents(), "n");
        terminals.write(id, b"\x1b[?1049halt\x1b[?1049l").unwrap();
        assert!(terminals
            .get(id)
            .unwrap()
            .screen()
            .contents()
            .starts_with("hi!"));
        terminals.reset(id).unwrap();
        assert_eq!(terminals.get(id).unwrap().screen().contents().trim(), "");
    }
    #[test]
    fn base64_writes_require_canonical_standard_encoding_within_bounds() {
        assert_eq!(decode_base64("AAE=").unwrap(), vec![0, 1]);
        assert!(decode_base64("!!!=").is_err());
        assert!(decode_base64("AAE").is_err());
        assert!(decode_base64("AB==").is_err());

        let boundary = STANDARD.encode(vec![0; MAX_WRITE_BYTES]);
        assert_eq!(decode_base64(&boundary).unwrap().len(), MAX_WRITE_BYTES);
        let oversized = STANDARD.encode(vec![0; MAX_WRITE_BYTES + 1]);
        assert!(decode_base64(&oversized).is_err());
    }

    #[test]
    fn modes_track_vt100_and_reset_with_the_parser() {
        let mut terminals = TerminalStore::new();
        let id = terminals.create(MAX_COLS, MAX_ROWS).unwrap();
        terminals.write(id, b"\x1b[?1h\x1b=\x1b[?2004h").unwrap();
        let modes = terminals.modes(id).unwrap();
        assert!(modes.application_cursor);
        assert!(modes.application_keypad);
        assert!(modes.bracketed_paste);

        terminals.reset(id).unwrap();
        let modes = terminals.modes(id).unwrap();
        assert!(!modes.application_cursor);
        assert!(!modes.application_keypad);
        assert!(!modes.bracketed_paste);
        assert!(terminals.create(MAX_COLS + 1, 1).is_err());
        assert!(terminals.create(1, MAX_ROWS + 1).is_err());
    }

    #[test]
    fn osc_is_only_parser_state() {
        let mut terminals = TerminalStore::new();
        let id = terminals.create(4, 1).unwrap();
        terminals
            .write(id, b"\x1b]0;not-a-host-title\x07ok")
            .unwrap();
        assert_eq!(terminals.get(id).unwrap().screen().contents(), "ok");
    }
}
