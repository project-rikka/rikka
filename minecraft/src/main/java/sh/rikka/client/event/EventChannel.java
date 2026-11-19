package sh.rikka.client.event;

import java.util.ArrayList;
import java.util.List;

public final class EventChannel<E extends Event> {
	private final Class<E> type;
	private final boolean cancellable;
	private final FailureHandler failureHandler;
	private final List<Registration<E>> registrations = new ArrayList<>();
	private volatile Snapshot<E> snapshot;

	EventChannel(Class<E> type, FailureHandler failureHandler) {
		this.type = type;
		this.cancellable = CancellableEvent.class.isAssignableFrom(type);
		this.failureHandler = failureHandler;
		this.snapshot = Snapshot.of(List.of());
	}

	public boolean hasListeners() {
		return this.snapshot.listeners.length != 0;
	}

	public E post(E event) {
		Snapshot<E> current = this.snapshot;
		if (this.cancellable) {
			dispatchCancellable(current, event, (CancellableEvent) event);
		} else {
			dispatch(current, event);
		}
		return event;
	}

	private void dispatch(Snapshot<E> current, E event) {
		Listener<? super E>[] listeners = current.listeners;
		for (int index = 0; index < listeners.length; index++) {
			try {
				listeners[index].handle(event);
			} catch (Exception failure) {
				this.failureHandler.onFailure(current.owners[index], event, failure);
			}
		}
	}

	private void dispatchCancellable(Snapshot<E> current, E event, CancellableEvent cancellable) {
		Listener<? super E>[] listeners = current.listeners;
		boolean[] receivesCancelled = current.receivesCancelled;
		for (int index = 0; index < listeners.length; index++) {
			if (cancellable.isCancelled() && !receivesCancelled[index]) {
				continue;
			}
			try {
				listeners[index].handle(event);
			} catch (Exception failure) {
				this.failureHandler.onFailure(current.owners[index], event, failure);
			}
		}
	}

	Subscription add(Listener<? super E> listener, Priority priority, boolean receivesCancelled, Object owner) {
		if (receivesCancelled && !this.cancellable) {
			throw new IllegalArgumentException(this.type.getName() + " is not cancellable");
		}
		Registration<E> registration = new Registration<>(listener, priority, receivesCancelled, owner);
		synchronized (this.registrations) {
			int index = 0;
			while (index < this.registrations.size() && this.registrations.get(index).priority.compareTo(priority) <= 0) {
				index++;
			}
			this.registrations.add(index, registration);
			this.snapshot = Snapshot.of(this.registrations);
		}
		return new Subscription(this, registration);
	}

	void remove(Object registration) {
		synchronized (this.registrations) {
			if (!this.registrations.remove(registration)) {
				throw new IllegalStateException("listener is not subscribed to " + this.type.getName());
			}
			this.snapshot = Snapshot.of(this.registrations);
		}
	}

	private static final class Registration<E extends Event> {
		private final Listener<? super E> listener;
		private final Priority priority;
		private final boolean receivesCancelled;
		private final Object owner;

		private Registration(Listener<? super E> listener, Priority priority, boolean receivesCancelled, Object owner) {
			this.listener = listener;
			this.priority = priority;
			this.receivesCancelled = receivesCancelled;
			this.owner = owner;
		}
	}

	private static final class Snapshot<E extends Event> {
		private final Listener<? super E>[] listeners;
		private final boolean[] receivesCancelled;
		private final Object[] owners;

		@SuppressWarnings("unchecked")
		private Snapshot(int size) {
			this.listeners = (Listener<? super E>[]) new Listener<?>[size];
			this.receivesCancelled = new boolean[size];
			this.owners = new Object[size];
		}

		private static <E extends Event> Snapshot<E> of(List<Registration<E>> registrations) {
			Snapshot<E> snapshot = new Snapshot<>(registrations.size());
			for (int index = 0; index < registrations.size(); index++) {
				Registration<E> registration = registrations.get(index);
				snapshot.listeners[index] = registration.listener;
				snapshot.receivesCancelled[index] = registration.receivesCancelled;
				snapshot.owners[index] = registration.owner;
			}
			return snapshot;
		}
	}
}
