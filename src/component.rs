use crate::{
    link::Link,
    dialect::MavComponent,
};

struct Component {
    id: MavComponent,
    system: u8,
    link: Link,
}

impl Component {
    fn new(id: MavComponent, system: u8, link: Link) -> Self {
        Self { id, system, link }
    }
}
