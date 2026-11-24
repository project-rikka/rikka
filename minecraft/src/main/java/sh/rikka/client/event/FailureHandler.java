package sh.rikka.client.event;

@FunctionalInterface
public interface FailureHandler {
	void onFailure(Object owner, Event event, Exception failure);
}
