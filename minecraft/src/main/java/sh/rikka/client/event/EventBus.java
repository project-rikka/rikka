package sh.rikka.client.event;

import java.lang.reflect.Modifier;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

public final class EventBus {
	private final FailureHandler failureHandler;
	private final ClassValue<EventChannel<?>> channels = new ClassValue<>() {
		@Override
		protected EventChannel<?> computeValue(Class<?> type) {
			if (!Event.class.isAssignableFrom(type) || !Modifier.isFinal(type.getModifiers())) {
				throw new IllegalArgumentException("event type must be a final Event: " + type.getName());
			}
			return createChannel(type.asSubclass(Event.class));
		}
	};
	private final Map<Object, Subscription[]> subscribers = new IdentityHashMap<>();

	public EventBus(FailureHandler failureHandler) {
		this.failureHandler = failureHandler;
	}

	private <E extends Event> EventChannel<E> createChannel(Class<E> type) {
		return new EventChannel<>(type, this.failureHandler);
	}

	@SuppressWarnings("unchecked")
	public <E extends Event> EventChannel<E> channel(Class<E> type) {
		return (EventChannel<E>) this.channels.get(type);
	}

	@SuppressWarnings("unchecked")
	public <E extends Event> E post(E event) {
		return ((EventChannel<E>) this.channels.get(event.getClass())).post(event);
	}

	public <E extends Event> Subscription listen(Class<E> type, Listener<? super E> listener) {
		return listen(type, Priority.NORMAL, listener);
	}

	public <E extends Event> Subscription listen(Class<E> type, Priority priority, Listener<? super E> listener) {
		return channel(type).add(listener, priority, false, listener);
	}

	public void subscribe(Object subscriber) {
		List<ListenerMethods.ListenerMethod> methods = ListenerMethods.of(subscriber.getClass());
		synchronized (this.subscribers) {
			if (this.subscribers.containsKey(subscriber)) {
				throw new IllegalStateException(subscriber + " is already subscribed");
			}
			Subscription[] subscriptions = new Subscription[methods.size()];
			for (int index = 0; index < subscriptions.length; index++) {
				ListenerMethods.ListenerMethod method = methods.get(index);
				subscriptions[index] = channel(method.eventType())
					.add(method.bind(subscriber), method.priority(), method.receiveCancelled(), subscriber);
			}
			this.subscribers.put(subscriber, subscriptions);
		}
	}

	public void unsubscribe(Object subscriber) {
		synchronized (this.subscribers) {
			Subscription[] subscriptions = this.subscribers.remove(subscriber);
			if (subscriptions == null) {
				throw new IllegalStateException(subscriber + " is not subscribed");
			}
			for (Subscription subscription : subscriptions) {
				subscription.unsubscribe();
			}
		}
	}
}
