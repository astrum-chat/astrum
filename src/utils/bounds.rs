use gpui::{Bounds, Length, point, px, relative, size};

pub trait BoundsExt {
    fn full() -> Self;
}

impl BoundsExt for Bounds<Length> {
    fn full() -> Self {
        Bounds {
            origin: point(px(0.).into(), px(0.).into()),
            size: size(relative(1.).into(), relative(1.).into()),
        }
    }
}
