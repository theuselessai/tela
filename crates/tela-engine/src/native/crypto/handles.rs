use super::{fail, CryptoRuntime, MAX_HANDLES};
use p256::{ProjectivePoint, Scalar};
use rquickjs::{class::Trace, Class, Ctx, JsLifetime, Result, Value};
use std::sync::{Arc, Mutex};
use zeroize::{Zeroize, Zeroizing};

pub(super) enum Material {
    Scalar(Scalar),
    Point(ProjectivePoint),
    Private(Zeroizing<[u8; 32]>),
    Key(Zeroizing<[u8; 32]>),
}
impl Drop for Material {
    fn drop(&mut self) {
        match self {
            Self::Scalar(s) => s.zeroize(),
            // Derived group elements may also be secret (e.g. PAKE shared points).
            Self::Point(p) => p.zeroize(),
            _ => {} // Secret byte fields are Zeroizing.
        }
    }
}

pub(super) struct Slot {
    context: usize,
    material: Mutex<Option<Material>>,
}
impl Slot {
    pub(super) fn dispose(&self) {
        self.material.lock().unwrap().take();
    }
}

macro_rules! handle {
    ($name:ident) => {
        #[derive(Trace, JsLifetime)]
        #[rquickjs::class]
        pub(super) struct $name {
            #[qjs(skip_trace)]
            slot: Arc<Slot>,
        }
        #[rquickjs::methods]
        impl $name {
            pub fn dispose(&self, ctx: Ctx<'_>) -> Result<()> {
                if self.slot.context != ctx.as_raw().as_ptr() as usize {
                    return Err(fail(&ctx, "crypto wrong context"));
                }
                self.slot.dispose();
                Ok(())
            }
        }
    };
}
handle!(P256Scalar);
handle!(P256Point);
handle!(X25519Private);
handle!(ChaChaKey);

pub(super) fn create<'js>(
    ctx: &Ctx<'js>,
    state: &Arc<CryptoRuntime>,
    material: Material,
) -> Result<Value<'js>> {
    let mut handles = state.handles.lock().unwrap();
    handles.retain(|weak| {
        weak.upgrade()
            .is_some_and(|s| s.material.lock().unwrap().is_some())
    });
    state.check(ctx)?;
    if handles.len() >= MAX_HANDLES {
        return Err(fail(ctx, "crypto handle limit exceeded"));
    }
    let kind = match &material {
        Material::Scalar(_) => 0,
        Material::Point(_) => 1,
        Material::Private(_) => 2,
        Material::Key(_) => 3,
    };
    let slot = Arc::new(Slot {
        context: state.context,
        material: Mutex::new(Some(material)),
    });
    handles.push(Arc::downgrade(&slot));
    // Release registry lock before allocating JS objects (GC may run).
    drop(handles);
    Ok(match kind {
        0 => Class::instance(ctx.clone(), P256Scalar { slot })?.into_value(),
        1 => Class::instance(ctx.clone(), P256Point { slot })?.into_value(),
        2 => Class::instance(ctx.clone(), X25519Private { slot })?.into_value(),
        _ => Class::instance(ctx.clone(), ChaChaKey { slot })?.into_value(),
    })
}

fn slot(ctx: &Ctx<'_>, value: &Value<'_>, kind: u8) -> Result<Arc<Slot>> {
    let object = value
        .as_object()
        .ok_or_else(|| fail(ctx, "crypto invalid handle"))?;
    macro_rules! get {
        ($t:ty) => {
            object.as_class::<$t>().map(|c| c.borrow().slot.clone())
        };
    }
    let slot = match kind {
        0 => get!(P256Scalar),
        1 => get!(P256Point),
        2 => get!(X25519Private),
        _ => get!(ChaChaKey),
    }
    .ok_or_else(|| fail(ctx, "crypto wrong handle kind"))?;
    if slot.context != ctx.as_raw().as_ptr() as usize {
        return Err(fail(ctx, "crypto wrong context"));
    }
    Ok(slot)
}

pub(super) fn scalar(ctx: &Ctx<'_>, value: &Value<'_>) -> Result<Zeroizing<Scalar>> {
    let slot = slot(ctx, value, 0)?;
    let guard = slot.material.lock().unwrap();
    match guard.as_ref() {
        Some(Material::Scalar(s)) => Ok(Zeroizing::new(*s)),
        _ => Err(fail(ctx, "crypto disposed handle")),
    }
}
pub(super) fn point(ctx: &Ctx<'_>, value: &Value<'_>) -> Result<ProjectivePoint> {
    let slot = slot(ctx, value, 1)?;
    let guard = slot.material.lock().unwrap();
    match guard.as_ref() {
        Some(Material::Point(p)) => Ok(*p),
        _ => Err(fail(ctx, "crypto disposed handle")),
    }
}
pub(super) fn secret(
    ctx: &Ctx<'_>,
    value: &Value<'_>,
    private: bool,
) -> Result<Zeroizing<[u8; 32]>> {
    let slot = slot(ctx, value, if private { 2 } else { 3 })?;
    let guard = slot.material.lock().unwrap();
    match guard.as_ref() {
        Some(Material::Private(s) | Material::Key(s)) => Ok(Zeroizing::new(**s)),
        _ => Err(fail(ctx, "crypto disposed handle")),
    }
}
