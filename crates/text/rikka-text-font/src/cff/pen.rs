use crate::outline::sink::OutlineSink;

pub(crate) struct Pen<'s, S> {
    sink: &'s mut S,
    transform: Option<[f32; 4]>,
    current: (f64, f64),
    open: bool,
}

impl<'s, S: OutlineSink> Pen<'s, S> {
    pub(crate) fn new(sink: &'s mut S, transform: Option<[f32; 4]>) -> Self {
        Self {
            sink,
            transform,
            current: (0.0, 0.0),
            open: false,
        }
    }

    #[inline]
    fn map(&self, x: f64, y: f64) -> (f32, f32) {
        let (x, y) = (x as f32, y as f32);
        match self.transform {
            None => (x, y),
            Some([a, b, c, d]) => (a * x + c * y, b * x + d * y),
        }
    }

    #[inline]
    fn begin(&mut self) {
        if !self.open {
            let (x, y) = self.map(self.current.0, self.current.1);
            self.sink.move_to(x, y);
            self.open = true;
        }
    }

    #[inline]
    pub(crate) fn move_to(&mut self, x: f64, y: f64) {
        self.close();
        self.current = (x, y);
    }

    #[inline]
    pub(crate) fn line_to(&mut self, x: f64, y: f64) {
        if (x, y) == self.current {
            return;
        }
        self.begin();
        let (px, py) = self.map(x, y);
        self.sink.line_to(px, py);
        self.current = (x, y);
    }

    #[inline]
    pub(crate) fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
        self.begin();
        let (ax, ay) = self.map(x1, y1);
        let (bx, by) = self.map(x2, y2);
        let (px, py) = self.map(x, y);
        self.sink.cubic_to(ax, ay, bx, by, px, py);
        self.current = (x, y);
    }

    #[inline]
    pub(crate) fn close(&mut self) {
        if self.open {
            self.sink.close();
            self.open = false;
        }
    }
}
