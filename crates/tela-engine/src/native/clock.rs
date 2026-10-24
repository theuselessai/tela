//! Monotonic, millisecond-resolution elapsed time for app measurements.
use rquickjs::{Ctx, Function, Object};
use std::time::Instant;

pub(crate) fn register_clock(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let origin = Instant::now();
    let performance = Object::new(ctx.clone())?;
    performance.set(
        "now",
        Function::new(ctx.clone(), move || origin.elapsed().as_millis() as f64)?,
    )?;
    ctx.globals().set("performance", performance)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_is_monotonic_and_not_the_wall_clock() {
        let rt = rquickjs::Runtime::new().unwrap();
        let context = rquickjs::Context::full(&rt).unwrap();
        context.with(|ctx| {
            register_clock(&ctx).unwrap();
            assert!(ctx.eval::<bool,_>("var a=performance.now(); Date.now=()=>-123; var b=performance.now(); Number.isSafeInteger(a)&&Number.isSafeInteger(b)&&a>=0&&b>=a&&b<1000").unwrap());
        });
    }
}
