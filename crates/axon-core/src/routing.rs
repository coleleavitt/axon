use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use crate::id::EndpointId;
use crate::route::{Route, Weight};
use crate::signal::Signal;

#[derive(Debug)]
pub struct RoutingTable<P> {
    routes: Vec<Route<P>>,
}

impl<P> RoutingTable<P> {
    pub const fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn push(&mut self, route: Route<P>) {
        self.routes.push(route);
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn routes(&self) -> impl Iterator<Item = &Route<P>> {
        self.routes.iter()
    }

    pub fn select<'a>(
        &'a self,
        from: &EndpointId,
        signal: &Signal<P>,
    ) -> Result<Option<&'a Route<P>>, RoutingError> {
        let mut selected = None;
        let mut ambiguous = false;

        for route in self
            .routes
            .iter()
            .filter(|route| route.from() == from && route.admits(signal))
        {
            match selected {
                None => {
                    selected = Some(route);
                    ambiguous = false;
                }
                Some(current) => match route.weight().cmp(&current.weight()) {
                    Ordering::Greater => {
                        selected = Some(route);
                        ambiguous = false;
                    }
                    Ordering::Equal => ambiguous = true,
                    Ordering::Less => {}
                },
            }
        }

        if ambiguous {
            match selected {
                Some(route) => Err(RoutingError::AmbiguousRoute {
                    from: from.clone(),
                    weight: route.weight(),
                }),
                None => Ok(None),
            }
        } else {
            Ok(selected)
        }
    }
}

impl<P> Default for RoutingTable<P> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingError {
    AmbiguousRoute { from: EndpointId, weight: Weight },
}

impl fmt::Display for RoutingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmbiguousRoute { from, weight } => {
                write!(
                    formatter,
                    "ambiguous admitted routes from {from} at weight {weight}"
                )
            }
        }
    }
}

impl Error for RoutingError {}
