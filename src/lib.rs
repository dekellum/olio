#![allow(dead_code)]

extern crate failure; // #[macro_use]
extern crate futures;
extern crate hyper;
extern crate tokio_core;
extern crate tempfile;

use failure::Error;

use std::io::{Error as IoError, ErrorKind, Write};
use futures::{Future, Stream};
use futures::future::err as f_err;
use hyper::Client;
use hyper::client::{FutureResponse, Response};
use tokio_core::reactor::Core;
use tempfile::tempfile;

fn resp_future(res: Response)
    -> Box<Future<Item=(), Error=hyper::Error> + Send>
{
    println!("Response: {}", res.status());
    println!("Headers:\n{}", res.headers());

    match tempfile() {
        Ok(mut tfile) => {
            let mut length_read: usize = 0;
            Box::new(res.body().for_each(move |chunk| {
                length_read += chunk.len();
                if length_read > 5_000 {
                    Err(IoError::new(
                        ErrorKind::Other,
                        format!("too long: {}+", length_read)
                    ).into())
                } else {
                    println!("chunk ({})", length_read);
                    tfile.write_all(&chunk).map_err(From::from)
                }
            }))
        }
        Err(e) => Box::new(f_err::<(), _>(e.into()))
    }
}

fn example() -> Result<(), Error> {
    let mut core = Core::new()?;
    let client = Client::new(&core.handle());

    // hyper::uri::Uri, via std String parse and FromStr
    let uri = "http://gravitext.com".parse()?;

    let fr: FutureResponse = client.get(uri); // FutureResponse

    // FnOnce(Response) -> IntoFuture<Error=hyper::Error>
    let work = fr.and_then(resp_future);

    core.run(work)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::example;

    #[test]
    fn text_example() {
        match example() {
            Ok(_) => println!("ok"),
            Err(e) => panic!("Error from work: {:?}", e)
        }
    }
}
