package sh.rikka.client.event;

public final class Subscription {
	private final EventChannel<?> channel;
	private final Object registration;

	Subscription(EventChannel<?> channel, Object registration) {
		this.channel = channel;
		this.registration = registration;
	}

	public void unsubscribe() {
		this.channel.remove(this.registration);
	}
}
