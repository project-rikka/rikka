package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import java.util.Collection;
import java.util.Collections;
import java.util.EnumSet;
import java.util.List;
import java.util.Set;
import net.minecraft.network.chat.Component;

public final class EnumSetSetting<E extends Enum<E>> extends Setting<Set<E>, EnumSetSetting<E>> {
	private final Class<E> type;
	private final List<E> constants;
	private final Set<E> defaultValue;
	private final Codec<Set<E>> codec;
	private volatile Set<E> value;

	EnumSetSetting(String id, String translationKey, EnumSet<E> defaultValue) {
		super(id, translationKey);
		EnumSet<E> constants = EnumSet.complementOf(defaultValue);
		constants.addAll(defaultValue);
		this.type = constants.iterator().next().getDeclaringClass();
		this.constants = List.copyOf(constants);
		this.defaultValue = copy(defaultValue);
		this.codec = EnumCodecs.of(this.type).listOf().xmap(this::copy, List::copyOf);
		this.value = this.defaultValue;
	}

	private Set<E> copy(Collection<E> constants) {
		EnumSet<E> set = EnumSet.noneOf(this.type);
		set.addAll(constants);
		return Collections.unmodifiableSet(set);
	}

	public Set<E> get() {
		return this.value;
	}

	public boolean contains(E constant) {
		return this.value.contains(constant);
	}

	public void set(Collection<E> constants) {
		Set<E> copied = copy(constants);
		if (this.value.equals(copied)) {
			return;
		}
		this.value = copied;
		changed(copied);
	}

	public void toggle(E constant) {
		EnumSet<E> next = EnumSet.noneOf(this.type);
		next.addAll(this.value);
		if (!next.remove(constant)) {
			next.add(constant);
		}
		set(next);
	}

	public List<E> constants() {
		return this.constants;
	}

	public Component constantName(E constant) {
		return translate("." + EnumCodecs.name(constant));
	}

	@Override
	public Set<E> value() {
		return this.value;
	}

	@Override
	public Set<E> defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(Set<E> value) {
		set(value);
	}

	@Override
	public Codec<Set<E>> codec() {
		return this.codec;
	}
}
