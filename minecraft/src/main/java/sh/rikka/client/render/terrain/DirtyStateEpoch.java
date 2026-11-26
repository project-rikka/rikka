package sh.rikka.client.render.terrain;

public final class DirtyStateEpoch {
	private static int current;

	private DirtyStateEpoch() {
	}

	public static int current() {
		return current;
	}

	public static void advance() {
		current++;
	}
}
