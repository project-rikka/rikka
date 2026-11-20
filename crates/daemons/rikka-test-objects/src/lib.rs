use std::collections::HashMap;
use std::sync::Arc;

use rikka_sync::{Mutex, MutexGuard};
use serde::Serialize;
use zbus::Message;
use zbus::blocking::{Connection, MessageIterator};
use zbus::message::{Header, Type};
use zbus::zvariant::{DynamicType, ObjectPath, OwnedValue, Value};

const OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const UNKNOWN_METHOD: &str = "org.freedesktop.DBus.Error.UnknownMethod";

type Values = HashMap<String, OwnedValue>;
type Interfaces = HashMap<String, Values>;
type Answer = Box<dyn Fn(&Header<'_>) -> zbus::Result<Message> + Send>;

#[derive(Default)]
struct Tree {
    objects: HashMap<String, Interfaces>,
    answers: HashMap<(String, String, String), Answer>,
    before_read: Vec<(String, String, String, OwnedValue)>,
}

pub struct FakeService {
    connection: Connection,
    name: String,
    root: String,
    tree: Arc<Mutex<Tree>>,
}

impl FakeService {
    #[must_use]
    pub fn serve(name: &str, root: &str) -> FakeService {
        let connection = Connection::session().expect("the private bus should accept a client");
        let tree = Arc::new(Mutex::new(Tree::default()));
        let calls = MessageIterator::from(&connection);
        let answering = (connection.clone(), root.to_owned(), Arc::clone(&tree));
        std::thread::Builder::new()
            .name("rikka-test-objects".to_owned())
            .spawn(move || {
                let (connection, root, tree) = answering;
                for message in calls.flatten() {
                    if message.message_type() == Type::MethodCall {
                        answer(&connection, &root, &tree, &message);
                    }
                }
            })
            .expect("the fake service thread should start");
        connection
            .request_name(name)
            .expect("the private bus should grant the name");
        FakeService {
            connection,
            name: name.to_owned(),
            root: root.to_owned(),
            tree,
        }
    }

    pub fn add<'v>(
        &self,
        path: &str,
        interface: &str,
        properties: impl IntoIterator<Item = (&'static str, Value<'v>)>,
    ) {
        let values: Values = properties
            .into_iter()
            .map(|(key, value)| (key.to_owned(), owned(value)))
            .collect();
        let announced = HashMap::from([(interface.to_owned(), copied(&values))]);
        self.tree()
            .objects
            .entry(path.to_owned())
            .or_default()
            .insert(interface.to_owned(), values);
        self.signal(
            &self.root,
            OBJECT_MANAGER,
            "InterfacesAdded",
            &(object_path(path), announced),
        );
    }

    pub fn remove(&self, path: &str, interface: &str) {
        let mut tree = self.tree();
        if let Some(object) = tree.objects.get_mut(path) {
            object.remove(interface);
            if object.is_empty() {
                tree.objects.remove(path);
            }
        }
        drop(tree);
        self.signal(
            &self.root,
            OBJECT_MANAGER,
            "InterfacesRemoved",
            &(object_path(path), vec![interface.to_owned()]),
        );
    }

    pub fn set(&self, path: &str, interface: &str, key: &str, value: Value<'_>) {
        let value = owned(value);
        let changed = HashMap::from([(key.to_owned(), copy(&value))]);
        self.store(path, interface, key, value);
        self.signal(
            path,
            PROPERTIES,
            "PropertiesChanged",
            &(interface, changed, Vec::<String>::new()),
        );
    }

    pub fn invalidate(&self, path: &str, interface: &str, key: &str, value: Value<'_>) {
        self.store(path, interface, key, owned(value));
        self.signal(
            path,
            PROPERTIES,
            "PropertiesChanged",
            &(interface, Values::new(), vec![key.to_owned()]),
        );
    }

    pub fn set_before_next_read(&self, path: &str, interface: &str, key: &str, value: Value<'_>) {
        self.tree().before_read.push((
            path.to_owned(),
            interface.to_owned(),
            key.to_owned(),
            owned(value),
        ));
    }

    pub fn answer<B>(&self, path: &str, interface: &str, member: &str, body: B)
    where
        B: Serialize + DynamicType + Send + 'static,
    {
        let reply: Answer = Box::new(move |call| Message::method_return(call)?.build(&body));
        self.tree().answers.insert(
            (path.to_owned(), interface.to_owned(), member.to_owned()),
            reply,
        );
    }

    pub fn signal<B>(&self, path: &str, interface: &str, member: &str, body: &B)
    where
        B: Serialize + DynamicType,
    {
        self.connection
            .emit_signal(None::<&str>, path, interface, member, body)
            .expect("the fake service should emit its signal");
    }

    pub fn leave(self) {
        self.connection
            .release_name(self.name.as_str())
            .expect("the fake service should release its name");
    }

    fn store(&self, path: &str, interface: &str, key: &str, value: OwnedValue) {
        self.tree()
            .objects
            .entry(path.to_owned())
            .or_default()
            .entry(interface.to_owned())
            .or_default()
            .insert(key.to_owned(), value);
    }

    fn tree(&self) -> MutexGuard<'_, Tree> {
        self.tree.lock()
    }
}

impl std::fmt::Debug for FakeService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FakeService")
            .field("name", &self.name)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Drop for FakeService {
    fn drop(&mut self) {
        if let Err(error) = self.connection.clone().close() {
            eprintln!("fake service connection did not close: {error}");
        }
    }
}

fn answer(connection: &Connection, root: &str, tree: &Mutex<Tree>, call: &Message) {
    let header = call.header();
    let path = header.path().map(|path| path.as_str()).unwrap_or_default();
    let interface = header
        .interface()
        .map(|interface| interface.as_str())
        .unwrap_or_default();
    let member = header
        .member()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let mut tree = tree.lock();
    let sent = match (interface, member) {
        (OBJECT_MANAGER, "GetManagedObjects") if path == root => {
            for (path, interface, key, value) in std::mem::take(&mut tree.before_read) {
                let changed = HashMap::from([(key.clone(), copy(&value))]);
                tree.objects
                    .entry(path.clone())
                    .or_default()
                    .entry(interface.clone())
                    .or_default()
                    .insert(key, value);
                connection
                    .emit_signal(
                        None::<&str>,
                        path.as_str(),
                        PROPERTIES,
                        "PropertiesChanged",
                        &(interface.as_str(), changed, Vec::<String>::new()),
                    )
                    .expect("the fake service should emit its signal");
            }
            let objects: HashMap<ObjectPath<'_>, Interfaces> = tree
                .objects
                .iter()
                .map(|(path, interfaces)| {
                    let interfaces = interfaces
                        .iter()
                        .map(|(name, values)| (name.clone(), copied(values)))
                        .collect();
                    (object_path(path), interfaces)
                })
                .collect();
            connection.reply(&header, &objects)
        }
        (PROPERTIES, "GetAll") => {
            let wanted: String = call
                .body()
                .deserialize()
                .expect("GetAll should name an interface");
            let values = tree
                .objects
                .get(path)
                .and_then(|object| object.get(&wanted))
                .map(copied)
                .unwrap_or_default();
            connection.reply(&header, &values)
        }
        _ => match tree
            .answers
            .get(&(path.to_owned(), interface.to_owned(), member.to_owned()))
        {
            Some(reply) => reply(&header).and_then(|reply| connection.send(&reply)),
            None => connection.reply_error(&header, UNKNOWN_METHOD, &member),
        },
    };
    if let Err(error) = sent {
        eprintln!("fake service failed to answer {interface}.{member}: {error}");
    }
}

fn object_path(path: &str) -> ObjectPath<'_> {
    ObjectPath::try_from(path).expect("fake object paths should be valid")
}

fn owned(value: Value<'_>) -> OwnedValue {
    OwnedValue::try_from(value).expect("fake values should hold no file descriptors")
}

fn copy(value: &OwnedValue) -> OwnedValue {
    value
        .try_clone()
        .expect("fake values should hold no file descriptors")
}

fn copied(values: &Values) -> Values {
    values
        .iter()
        .map(|(key, value)| (key.clone(), copy(value)))
        .collect()
}
