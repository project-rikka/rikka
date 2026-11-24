package sh.rikka.client.event;

public abstract class CancellableEvent implements Event {
	private boolean cancelled;

	protected CancellableEvent() {
	}

	public final boolean isCancelled() {
		return this.cancelled;
	}

	public final void cancel() {
		this.cancelled = true;
	}

	public final void setCancelled(boolean cancelled) {
		this.cancelled = cancelled;
	}
}
