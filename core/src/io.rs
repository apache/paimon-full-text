use std::io;

pub struct ReadRequest<'a> {
    pub pos: u64,
    pub buf: &'a mut [u8],
}

pub trait SeekRead: Send {
    fn pread(&mut self, ranges: &mut [ReadRequest<'_>]) -> io::Result<()>;
}

pub trait SeekWrite: Send {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct PosWriter<W> {
    inner: W,
}

impl<W> PosWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: io::Write + Send> SeekWrite for PosWriter<W> {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        io::Write::write_all(&mut self.inner, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut self.inner)
    }
}

pub struct SliceReader {
    data: Vec<u8>,
}

impl SliceReader {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl SeekRead for SliceReader {
    fn pread(&mut self, ranges: &mut [ReadRequest<'_>]) -> io::Result<()> {
        for range in ranges {
            let start = usize::try_from(range.pos)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
            let end = start
                .checked_add(range.buf.len())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
            if end > self.data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "read past end of slice",
                ));
            }
            range.buf.copy_from_slice(&self.data[start..end]);
        }
        Ok(())
    }
}
