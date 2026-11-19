package sh.rikka.client.event;

@FunctionalInterface
public interface Listener<E extends Event> {
	void handle(E event);
}
