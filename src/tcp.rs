use std::io::{self, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs, TcpListener, TcpStream, Shutdown};

use openssl::ssl::{self, SslContext, SslMethod, Ssl, SslStream};
use openssl::dh::DH;
use openssl::x509::X509FileType;

use super::types::{CaesarError, Result};

#[derive(Debug)]
pub struct TlsTcpListener {
    listener: TcpListener,
    ctx: SslContext,
}

#[derive(Debug)]
pub struct TlsTcpStream {
    stream: SslStream<TcpStream>,
}

#[derive(Debug)]
pub struct Incoming<'a> {
    listener: &'a TlsTcpListener,
}

impl TlsTcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A, key: &str, cert: &str) -> Result<TlsTcpListener> {
        // create listener
        let listener = try!(TcpListener::bind(addr).map_err(|e| CaesarError::Io(e)));
        let ctx = try!(new_ssl_context(&key, &cert));
        Ok(TlsTcpListener {
            listener: listener,
            ctx: ctx,
        })
    }

    pub fn accept(&self) -> Result<(TlsTcpStream, SocketAddr)> {
        // acept from bare TCP stream
        let (stream, addr) = try!(self.listener.accept().map_err(|e| CaesarError::Io(e)));
        // create SSL object with stored context
        let ssl = try!(Ssl::new(&self.ctx).map_err(|e| CaesarError::Ssl(e)));
        // accept from encrypted stream
        let tls_stream = try!(SslStream::accept(ssl, stream).map_err(|e| CaesarError::Ssl(e)));
        Ok((TlsTcpStream { stream: tls_stream }, addr))
    }

    pub fn incoming(&self) -> Incoming {
        Incoming { listener: self }
    }
}

impl<'a> Iterator for Incoming<'a> {
    type Item = Result<TlsTcpStream>;
    fn next(&mut self) -> Option<Result<TlsTcpStream>> {
        Some(self.listener.accept().map(|p| p.0))
    }
}

impl TlsTcpStream {
    pub fn peer_addr(&self) -> Result<SocketAddr> {
        self.stream.get_ref().peer_addr().map_err(|e| CaesarError::Io(e))
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.stream.get_ref().shutdown(how)
    }
}

impl Read for TlsTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for TlsTcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

fn new_ssl_context(key: &str, cert: &str) -> Result<SslContext> {
    let mut ctx = try!(ssl::SslContext::new(SslMethod::Tlsv1_2).map_err(|e| CaesarError::Ssl(e)));
    // use recommended settings
    let opts = ssl::SSL_OP_CIPHER_SERVER_PREFERENCE | ssl::SSL_OP_NO_COMPRESSION |
               ssl::SSL_OP_NO_TICKET | ssl::SSL_OP_NO_SSLV2 |
               ssl::SSL_OP_NO_SSLV3 | ssl::SSL_OP_NO_TLSV1 | ssl::SSL_OP_NO_TLSV1_1;
    ctx.set_options(opts);
    // set strong suite of ciphers
    try!(ctx.set_cipher_list("ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:\
                              ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:\
                              ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:\
                              DHE-RSA-AES128-GCM-SHA256:DHE-RSA-AES256-GCM-SHA384:\
                              ECDHE-ECDSA-AES128-SHA256:ECDHE-RSA-AES128-SHA256:\
                              ECDHE-ECDSA-AES128-SHA:ECDHE-RSA-AES256-SHA384:\
                              ECDHE-RSA-AES128-SHA:ECDHE-ECDSA-AES256-SHA384:\
                              ECDHE-ECDSA-AES256-SHA:ECDHE-RSA-AES256-SHA:DHE-RSA-AES128-SHA256:\
                              DHE-RSA-AES128-SHA:DHE-RSA-AES256-SHA256:DHE-RSA-AES256-SHA:\
                              ECDHE-ECDSA-DES-CBC3-SHA:ECDHE-RSA-DES-CBC3-SHA:\
                              EDH-RSA-DES-CBC3-SHA:AES128-GCM-SHA256:AES256-GCM-SHA384:\
                              AES128-SHA256:AES256-SHA256:AES128-SHA:AES256-SHA:DES-CBC3-SHA:\
                              !DSS")
            .map_err(|e| CaesarError::Ssl(e)));
    // enable forward secrecy
    let dh = try!(DH::get_2048_256().map_err(|e| CaesarError::Ssl(e)));
    try!(ctx.set_tmp_dh(dh).map_err(|e| CaesarError::Ssl(e)));
    // set cert and key files
    try!(ctx.set_private_key_file(key, X509FileType::PEM).map_err(|e| CaesarError::Ssl(e)));
    try!(ctx.set_certificate_file(cert, X509FileType::PEM).map_err(|e| CaesarError::Ssl(e)));
    // check integrity
    try!(ctx.check_private_key().map_err(|e| CaesarError::Ssl(e)));
    Ok(ctx)
}
