package sh.rikka.client.mod;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import sh.rikka.client.event.EventBus;

public final class ModManager {
	private final EventBus bus;
	private final List<Mod> mods = new ArrayList<>();
	private final List<Mod> view = Collections.unmodifiableList(this.mods);
	private final Map<String, Mod> byId = new HashMap<>();
	private final Map<Class<? extends Mod>, Mod> byType = new HashMap<>();
	private volatile boolean locked;

	public ModManager(EventBus bus) {
		this.bus = bus;
	}

	public <M extends Mod> M register(M mod) {
		if (this.locked) {
			throw new IllegalStateException("mods are locked; register " + mod.id() + " during initialization");
		}
		if (this.byId.containsKey(mod.id())) {
			throw new IllegalArgumentException("duplicate mod id: " + mod.id());
		}
		if (this.byType.containsKey(mod.getClass())) {
			throw new IllegalArgumentException("duplicate mod type: " + mod.getClass().getName());
		}
		mod.attach(this.bus);
		this.mods.add(mod);
		this.byId.put(mod.id(), mod);
		this.byType.put(mod.getClass(), mod);
		return mod;
	}

	public void lock() {
		for (Mod mod : this.mods) {
			mod.settings().lock();
		}
		this.locked = true;
	}

	public boolean isLocked() {
		return this.locked;
	}

	public List<Mod> all() {
		return this.view;
	}

	public Optional<Mod> find(String id) {
		return Optional.ofNullable(this.byId.get(id));
	}

	public <M extends Mod> M get(Class<M> type) {
		Mod mod = this.byType.get(type);
		if (mod == null) {
			throw new IllegalArgumentException("mod is not registered: " + type.getName());
		}
		return type.cast(mod);
	}

	public List<Mod> inCategory(Category category) {
		return this.mods.stream().filter(mod -> mod.category() == category).toList();
	}
}
