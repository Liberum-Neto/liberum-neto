use super::super::*;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct PinObject {
    pub from: ObjectId,
    pub to: SignedObject,
}
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct PinContext {
    pub direction: PinContextDirection,
    // Jaki jest kierunek relacji (przykład: Upgrade vs Downgrade)
    pub relation: ObjectId,
    // Jaka to relacja? (przykład: Upgrade vs Like)
    // Używa do tego tagu, choć nie wszystkie tagi mogą mieć sens jako kontekst
    pub object: SignedObject,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum PinContextDirection {
    None,
    From, // Od `object` do `related`
    To,   // Od `related` do `object`
}
