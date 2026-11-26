package sh.rikka.client.mods.boost;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

public enum BoostFeature {
	GPU_ENTITIES("gpu_entities", true),
	DISTANT_ENTITIES("distant_entities", true),
	ENTITY_CULLING("entity_culling", true),
	LEAF_CULLING("leaf_culling", true),
	FOG_CULLING("fog_culling", true),
	FAST_FILTERING("fast_filtering", true),
	COMPACT_VERTICES("compact_vertices", true),
	RENDER_SCALE("render_scale", false);

	private final String id;
	private final boolean enabledByDefault;
	private final List<Runnable> listeners = new CopyOnWriteArrayList<>();
	private volatile boolean active;

	BoostFeature(String id, boolean enabledByDefault) {
		this.id = id;
		this.enabledByDefault = enabledByDefault;
	}

	public String id() {
		return this.id;
	}

	boolean enabledByDefault() {
		return this.enabledByDefault;
	}

	public boolean isActive() {
		return this.active;
	}

	public void onChange(Runnable listener) {
		this.listeners.add(listener);
	}

	void setActive(boolean active) {
		if (this.active == active) {
			return;
		}
		this.active = active;
		for (Runnable listener : this.listeners) {
			listener.run();
		}
	}
}
