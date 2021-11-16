use std::future::Future;

use actix_service::{
    boxed::{self, BoxServiceFactory},
    fn_service, Service,
};

use crate::{
    extract::FromRequestX,
    service::{ServiceRequest, ServiceResponse},
    Error, HttpRequest, HttpResponse, Responder,
};

// TODO inaccessible docs
/// A request handler is an async function that accepts zero or more parameters that can be
/// extracted from a request (i.e., [`impl FromRequest`](crate::FromRequest)) and returns a type
/// that can be converted into an [`HttpResponse`] (that is, it impls the [`Responder`] trait).
///
/// If you got the error `the trait Handler<_, _, _> is not implemented`, then your function is not
/// a valid handler. See [Request Handlers](https://actix.rs/docs/handlers/) for more information.
pub trait Handler<'a, T: FromRequestX<'a>>: Clone + 'static {
    // TODO why 'static ??
    type Response: Responder + 'static;
    type HandlerFuture: Future<Output = Self::Response>;

    fn handle(&'a self, _: T::Output) -> Self::HandlerFuture;
}

impl<'a, F, T, Fut, Resp> Handler<'a, T> for F
where
    F: FnX<T>,
    F: FnX<T::Output, Output = Fut>,
    F: Clone + 'static,
    T: FromRequestX<'a>,
    Fut: Future<Output = Resp>,
    Resp: Responder + 'static,
{
    type Response = Resp;
    //TODO remove Handler
    type HandlerFuture = Fut;

    fn handle(&'a self, data: T::Output) -> Self::HandlerFuture {
        self.call(data)
    }
}

pub fn handler_service<H, T>(
    handler: H,
) -> BoxServiceFactory<(), ServiceRequest, ServiceResponse, Error, ()>
where
    H: for<'a> Handler<'a, T>,
    T: for<'a> FromRequestX<'a>,
{
    boxed::factory(fn_service(move |req: ServiceRequest| {
        let handler = handler.clone();
        async move {
            let (req, mut payload) = req.into_parts();
            let res = match T::from_request_x(&req, &mut payload).await {
                Err(err) => HttpResponse::from_error(err),
                Ok(data) => handler.handle(data).await.respond_to(&req),
            };
            Ok(ServiceResponse::new(req, res))
        }
    }))
}

/// Same as [`std::ops::Fn`]
pub trait FnX<Args> {
    type Output;
    fn call(&self, args: Args) -> Self::Output;
}

/// FromRequest trait impl for tuples
macro_rules! fn_tuple ({ $($param:ident)* } => {
    impl<Func, $($param,)* O> FnX<($($param,)*)> for Func
    where Func: Fn($($param),*) -> O,
    {
        type Output = O;

        #[allow(non_snake_case)]
        fn call(&self, ($($param,)*): ($($param,)*)) -> O {
            (self)($($param,)*)
        }
    }
});

fn_tuple! {}
fn_tuple! { A }
fn_tuple! { A B }
fn_tuple! { A B C }
fn_tuple! { A B C D }
fn_tuple! { A B C D E }
fn_tuple! { A B C D E F }
fn_tuple! { A B C D E F G }
fn_tuple! { A B C D E F G H }
fn_tuple! { A B C D E F G H I }
fn_tuple! { A B C D E F G H I J }
fn_tuple! { A B C D E F G H I J K }
fn_tuple! { A B C D E F G H I J K L }

mod test {
    use super::*;

    type Req = HttpRequest;

    fn check<T, F, R>(
        req: ServiceRequest,
        f: F,
    ) -> impl Future<Output = Result<R, Error>> + 'static
    where
        F: for<'a> Handler<'a, T, Response = R>,
        T: for<'a> FromRequestX<'a>,
    {
        async move {
            let (req, mut payload) = req.into_parts();
            let data = T::from_request_x(&req, &mut payload)
                .await
                .map_err(|e| e.into())?;
            Ok(f.handle(data).await)
        }
    }

    async fn handler(_: &Req, _: (&Req, &Req)) -> &'static str {
        "hello"
    }

    fn test(req: ServiceRequest) {
        check(req, handler);

        //check(req, |_: (&Req, &Req), _: &Req| async { "hello" });
    }
}
