use std::io::Write;

pub struct MultiWriter<W1, W2> {
    one: W1,
    two: W2,
}

impl<W1, W2> MultiWriter<W1, W2> {
    pub fn new(one: W1, two: W2) -> Self {
        Self {
            one,
            two,
        }
    }
}

impl<W1: Write, W2: Write> Write for MultiWriter<W1, W2> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let one = self.one.write(buf);
        let two = self.two.write(buf);

        one.and(two)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let one = self.one.flush();
        let two = self.two.flush();

        one.and(two)
    }
}