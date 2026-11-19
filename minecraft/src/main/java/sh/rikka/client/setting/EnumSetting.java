package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import java.util.List;
import net.minecraft.network.chat.Component;

public final class EnumSetting<E extends Enum<E>> extends Setting<E, EnumSetting<E>> {
	private final List<E> constants;
	private final E defaultValue;
	private final Codec<E> codec;
	private volatile E value;

	EnumSetting(String id, String translationKey, E defaultValue) {
		super(id, translationKey);
		Class<E> type = defaultValue.getDeclaringClass();
		this.constants = List.of(type.getEnumConstants());
		this.defaultValue = defaultValue;
		this.codec = EnumCodecs.of(type);
		this.value = defaultValue;
	}

	public E get() {
		return this.value;
	}

	public boolean is(E constant) {
		return this.value == constant;
	}

	public void set(E value) {
		if (this.value == value) {
			return;
		}
		this.value = value;
		changed(value);
	}

	public void cycle() {
		set(this.constants.get((this.value.ordinal() + 1) % this.constants.size()));
	}

	public List<E> constants() {
		return this.constants;
	}

	public Component constantName(E constant) {
		return translate("." + EnumCodecs.name(constant));
	}

	@Override
	public E value() {
		return this.value;
	}

	@Override
	public E defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(E value) {
		set(value);
	}

	@Override
	public Codec<E> codec() {
		return this.codec;
	}
}
