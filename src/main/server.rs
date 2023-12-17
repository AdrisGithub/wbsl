use std::io;
use std::io::Write;
use std::net::{Incoming, SocketAddr, TcpListener, TcpStream};

use aul::error;
use aul::level::Level;
use aul::log;
use whdp::{Request, TryRequest};
use whdp::resp_presets::{internal_server_error, not_found};

use crate::helper::{AdditionalHeaders, Logger};
use crate::middleware::Middleware;
use crate::router::Router;

pub struct Server {
    listener: TcpListener,
    middlewares: Vec<Box<dyn Middleware>>,
    router: Router,
}

pub struct ServerBuilder {
    socket: Option<SocketAddr>,
    middlewares: Vec<Box<dyn Middleware>>,
    router: Router,
}

impl ServerBuilder {
    pub fn new() -> Self {
        ServerBuilder::default()
    }
    pub fn add_middle(&mut self, middle: Box<dyn Middleware>) -> &mut Self {
        self.middlewares.push(middle);
        self
    }

    pub fn with_logging(&mut self, level: Level) -> &mut Self {
        self.add_middle(Box::new(Logger::from(level)))
    }
    pub fn with_auto_headers(&mut self, app_name: String, content_type: String) -> &mut Self {
        self.add_middle(Box::new(AdditionalHeaders::from((app_name, content_type))))
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self {
            socket: None,
            middlewares: Vec::new(),
            router: Router::new(),
        }
    }
}

impl Server {
    pub fn start(&self) {
        for (mut req, mut stream) in self {
            for middle in self.middlewares.iter() {
                req = middle.on_request(req);
            }

            // do the routing shit
            let func = self.router.get_func(req.get_uri(), req.get_method());

            let mut resp;

            if let Some(func) = func {
                resp = func(req)
            } else {
                resp = not_found(String::new())
            }

            for middle in self.middlewares.iter() {
                resp = middle.on_response(resp)
            }

            let _ = stream.write_all(resp.to_string().as_bytes());
        }
    }

    pub fn incoming(&self) -> Incoming<'_> {
        self.listener.incoming()
    }
}

impl TryFrom<SocketAddr> for Server {
    type Error = io::Error;
    fn try_from(value: SocketAddr) -> Result<Self, Self::Error> {
        Ok(Self {
            listener: TcpListener::bind(value)?,
            middlewares: Vec::new(),
            router: Router::new(),
        })
    }
}

impl Iterator for &Server {
    type Item = (Request, TcpStream);
    fn next(&mut self) -> Option<Self::Item> {
        let opt = self.incoming().next();
        if let Some(res) = opt {
            if let Ok(mut stream) = res {
                let res_req = stream.try_to_request();
                if let Ok(req) = res_req {
                    return Some((req, stream));
                } else {
                    let _ = stream.write_all(internal_error().as_bytes());
                    error!("Error parsing Request: {}", res_req.err().unwrap())
                }
            } else {
                error!("Error establishing Connection: {}", res.err().unwrap())
            }
        }
        self.next()
    }
}

fn internal_error() -> String {
    internal_server_error("Internal Server Error".into()).to_string()
}
