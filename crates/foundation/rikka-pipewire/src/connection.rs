use std::cell::Cell;
use std::ops::ControlFlow;
use std::os::fd::OwnedFd;
use std::rc::Rc;
use std::time::Duration;

use pipewire::channel::Receiver;
use pipewire::context::ContextRc;
use pipewire::core::{CoreRc, PW_ID_CORE};
use pipewire::main_loop::MainLoopRc;
use pipewire::registry::RegistryRc;

pub struct Connection {
    registry: RegistryRc,
    core: CoreRc,
    _context: ContextRc,
    main_loop: MainLoopRc,
}

impl Connection {
    pub fn open() -> Result<Connection, pipewire::Error> {
        Connection::dial(|context| context.connect_rc(None))
    }

    pub fn open_fd(remote: OwnedFd) -> Result<Connection, pipewire::Error> {
        Connection::dial(|context| context.connect_fd_rc(remote, None))
    }

    fn dial(
        connect: impl FnOnce(&ContextRc) -> Result<CoreRc, pipewire::Error>,
    ) -> Result<Connection, pipewire::Error> {
        pipewire::init();
        let main_loop = MainLoopRc::new(None)?;
        let context = ContextRc::new(&main_loop, None)?;
        let core = connect(&context)?;
        let registry = core.get_registry_rc()?;
        Ok(Connection {
            registry,
            core,
            _context: context,
            main_loop,
        })
    }

    #[must_use]
    pub fn main_loop(&self) -> &MainLoopRc {
        &self.main_loop
    }

    #[must_use]
    pub fn core(&self) -> &CoreRc {
        &self.core
    }

    #[must_use]
    pub fn registry(&self) -> &RegistryRc {
        &self.registry
    }

    pub fn run_with<C: 'static>(
        &self,
        commands: Receiver<C>,
        act: impl Fn(C) -> ControlFlow<()> + 'static,
    ) {
        let stopping = self.main_loop.downgrade();
        let _commands = commands.attach(self.main_loop.loop_(), move |command| {
            if act(command).is_break()
                && let Some(main_loop) = stopping.upgrade()
            {
                main_loop.quit();
            }
        });
        self.main_loop.run();
    }

    pub fn run_until_stopped(&self, stopping: Receiver<()>) {
        self.run_with(stopping, |()| ControlFlow::Break(()));
    }

    pub fn settle(&self, timeout: Duration) -> Result<bool, pipewire::Error> {
        let pending = self.core.sync(0)?;
        let settled = Rc::new(Cell::new(false));
        let done = Rc::clone(&settled);
        let finishing = self.main_loop.downgrade();
        let _core = self
            .core
            .add_listener_local()
            .done(move |id, seq| {
                if id == PW_ID_CORE
                    && seq == pending
                    && let Some(main_loop) = finishing.upgrade()
                {
                    done.set(true);
                    main_loop.quit();
                }
            })
            .register();
        let expiring = self.main_loop.downgrade();
        let timer = self.main_loop.loop_().add_timer(move |_| {
            if let Some(main_loop) = expiring.upgrade() {
                main_loop.quit();
            }
        });
        timer.update_timer(Some(timeout), None).into_sync_result()?;
        self.main_loop.run();
        Ok(settled.get())
    }
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Connection").finish_non_exhaustive()
    }
}
