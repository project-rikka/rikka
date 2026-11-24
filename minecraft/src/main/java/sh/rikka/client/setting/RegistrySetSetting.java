package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import it.unimi.dsi.fastutil.objects.ReferenceOpenHashSet;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.Registry;
import net.minecraft.resources.Identifier;

public final class RegistrySetSetting<T> extends Setting<Set<Identifier>, RegistrySetSetting<T>> {
	private static final Codec<Set<Identifier>> CODEC = Identifier.CODEC.listOf().xmap(RegistrySetSetting::copy, List::copyOf);

	private final Registry<T> registry;
	private final Set<Identifier> defaultValue;
	private volatile Selection<T> selection;

	RegistrySetSetting(String id, String translationKey, Registry<T> registry, Collection<T> defaultValue) {
		super(id, translationKey);
		this.registry = registry;
		Set<Identifier> ids = new LinkedHashSet<>();
		for (T entry : defaultValue) {
			ids.add(key(registry, entry));
		}
		this.defaultValue = Collections.unmodifiableSet(ids);
		this.selection = Selection.of(registry, this.defaultValue);
	}

	private static Set<Identifier> copy(Collection<Identifier> ids) {
		return Collections.unmodifiableSet(new LinkedHashSet<>(ids));
	}

	private static <T> Identifier key(Registry<T> registry, T entry) {
		Identifier key = registry.getKey(entry);
		if (key == null) {
			throw new IllegalArgumentException(entry + " is not registered in " + registry.key().identifier());
		}
		return key;
	}

	public Registry<T> registry() {
		return this.registry;
	}

	public Set<Identifier> get() {
		return this.selection.ids;
	}

	public boolean contains(T entry) {
		return this.selection.entries.contains(entry);
	}

	public void set(Collection<Identifier> ids) {
		Set<Identifier> copied = copy(ids);
		if (this.selection.ids.equals(copied)) {
			return;
		}
		this.selection = Selection.of(this.registry, copied);
		changed(copied);
	}

	public void toggle(T entry) {
		Identifier key = key(this.registry, entry);
		Set<Identifier> next = new LinkedHashSet<>(this.selection.ids);
		if (!next.remove(key)) {
			next.add(key);
		}
		set(next);
	}

	@Override
	public Set<Identifier> value() {
		return this.selection.ids;
	}

	@Override
	public Set<Identifier> defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(Set<Identifier> value) {
		set(value);
	}

	@Override
	public Codec<Set<Identifier>> codec() {
		return CODEC;
	}

	private record Selection<T>(Set<Identifier> ids, Set<T> entries) {
		private static <T> Selection<T> of(Registry<T> registry, Set<Identifier> ids) {
			ReferenceOpenHashSet<T> entries = new ReferenceOpenHashSet<>(ids.size());
			for (Identifier id : ids) {
				registry.getOptional(id).ifPresent(entries::add);
			}
			return new Selection<>(ids, entries);
		}
	}
}
