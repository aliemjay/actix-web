//! Middlware helper.

use std::{fmt, future::Future, rc::Rc};

use actix_utils::future::{ready, Ready};
use futures_core::future::LocalBoxFuture;

use crate::{
    body::MessageBody,
    dev::{Payload, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpRequest, HttpResponse, Responder,
};

struct Helper<F>(F);

impl<F> Helper<F> {
    fn new(f: F) -> Self
where
        //Self: Transform<S, ServiceRequest>,
    {
        Self(f)
    }
}

impl<S, F, Fut, R> Transform<S, ServiceRequest> for Helper<F>
where
    Rc<S>: NextService,
    F: Fn(HttpRequest, Payload, Rc<S>) -> Fut + Clone,
    Fut: Future<Output = Result<R, HelperError>>,
    R: Responder,
{
    type Response = ServiceResponse<R::Body>;
    type Error = Error;
    type Transform = HelperService<F, Rc<S>>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(HelperService {
            handler: self.0.clone(),
            next: Rc::new(service),
        }))
    }
}

pub struct HelperService<F, S> {
    handler: F,
    next: S,
}

impl<F, S, Fut, R> Service<ServiceRequest> for HelperService<F, S>
where
    S: NextService + Clone,
    F: Fn(HttpRequest, Payload, S) -> Fut,
    Fut: Future<Output = Result<R, HelperError>>,
    R: Responder,
{
    type Response = ServiceResponse<R::Body>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_service::always_ready!();

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let (req, payload) = req.into_parts();
        let _fut = (self.handler)(req, payload, self.next.clone());

        Box::pin(async move {});
        unimplemented!()
    }
}

#[derive(Debug)]
pub enum HelperError {
    Fatal(Error),
    Responsive(Error),
}

impl<T: Into<Error>> From<T> for HelperError {
    fn from(e: T) -> Self {
        Self::Responsive(e.into())
    }
}

impl fmt::Display for HelperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fatal(e) => fmt::Display::fmt(e, f),
            Self::Responsive(e) => fmt::Display::fmt(e, f),
        }
    }
}

pub trait NextService: 'static {
    type Body: MessageBody;
    type Future: Future<Output = Result<HttpResponse<Self::Body>, HelperError>>;

    fn call(&self, req: &mut HttpRequest, payload: Payload) -> Self::Future;
}

impl<S, B> NextService for Rc<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody,
{
    type Body = B;
    type Future = LocalBoxFuture<'static, Result<HttpResponse<B>, HelperError>>;

    fn call(&self, req: &mut HttpRequest, payload: Payload) -> Self::Future {
        let fut = Service::call(self, ServiceRequest::new(req.clone(), payload));

        Box::pin(async move {
            let resp = fut.await.map_err(HelperError::Fatal)?;
            Ok(resp.into_parts().1)
        })
    }
}

//#[cfg(test)]
mod tests {
    use actix_service::Service;

    use super::*;
    use crate::{
        http::StatusCode,
        service::{ServiceRequest, ServiceResponse},
        test::{init_service, TestRequest},
        App, HttpRequest,
    };

    //#[actix_rt::test]
    async fn test_compile() {
        async fn mw(
            mut req: HttpRequest,
            payload: Payload,
            next: impl NextService,
        ) -> Result<impl Responder, HelperError> {
            Ok(next.call(&mut req, payload).await?)
        }

        fn return_from_fn<S, B>() -> impl Transform<
            S,
            ServiceRequest,
            Response = ServiceResponse<impl MessageBody>,
            Error = Error,
            InitError = (),
        >
        where
            S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
            B: MessageBody,
        {
            Helper::new(mw)
        }

        let srv = init_service(
            App::new()
                //.wrap(Helper::new(|req, pl, next| async move { next.call(&mut req, pl).await }))
                .wrap(Helper::new(|req, pl, next| async move {
                    mw(req, pl, next).await
                }))
                .wrap(return_from_fn())
                .wrap(Helper::new(mw)),
        )
        .await;

        let req = TestRequest::with_uri("/app").to_request();
        let resp = srv.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let req = TestRequest::with_uri("/app/").to_request();
        let resp = srv.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
