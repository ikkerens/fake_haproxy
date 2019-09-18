use std::{
	io::{self, Read, Write},
	net::Shutdown,
	sync::Arc,
};

use futures::Poll;
use tokio_core::net::TcpStream;
use tokio_io::{AsyncRead, AsyncWrite};

/// A TcpStream with support for shutting down via a method instead of just being deallocated.
/// This is used when one side of the proxy shuts down,
/// so that the other side can also be shut down, thus ending the event loop.
#[derive(Clone)]
pub(super) struct AsyncTcpStream(Arc<TcpStream>);

impl From<TcpStream> for AsyncTcpStream {
	fn from(s: TcpStream) -> Self {
		AsyncTcpStream(Arc::new(s))
	}
}

impl Read for AsyncTcpStream {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		(&*self.0).read(buf)
	}
}

impl Write for AsyncTcpStream {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		(&*self.0).write(buf)
	}

	fn flush(&mut self) -> io::Result<()> {
		Ok(())
	}
}

impl AsyncRead for AsyncTcpStream {}

impl AsyncWrite for AsyncTcpStream {
	fn shutdown(&mut self) -> Poll<(), io::Error> {
		self.0.shutdown(Shutdown::Write)?;
		Ok(().into())
	}
}
