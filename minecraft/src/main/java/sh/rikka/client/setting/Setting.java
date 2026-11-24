package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.BooleanSupplier;
import java.util.function.Consumer;
import net.minecraft.locale.Language;
import net.minecraft.network.chat.Component;
import sh.rikka.client.RikkaClient;

public abstract sealed class Setting<T, S extends Setting<T, S>>
	permits BoolSetting, IntSetting, DoubleSetting, ColorSetting, StringSetting, EnumSetting, KeybindSetting, IntRangeSetting,
	DoubleRangeSetting, EnumSetSetting, RegistrySetSetting, StringListSetting, PositionSetting {
	private static final String SHARED_TRANSLATION_PREFIX = RikkaClient.MOD_ID + ".setting.";
	private static final BooleanSupplier ALWAYS = () -> true;

	private final String id;
	private final String translationKey;
	private final List<Consumer<? super T>> listeners = new CopyOnWriteArrayList<>();
	private volatile BooleanSupplier visibility = ALWAYS;

	Setting(String id, String translationKey) {
		this.id = id;
		this.translationKey = translationKey;
	}

	public final String id() {
		return this.id;
	}

	public final String translationKey() {
		return this.translationKey;
	}

	public final Component name() {
		return translate("");
	}

	public final Component description() {
		return translate(".description");
	}

	final Component translate(String suffix) {
		String key = this.translationKey + suffix;
		return Language.getInstance().has(key) ? Component.translatable(key) : Component.translatable(SHARED_TRANSLATION_PREFIX + this.id + suffix);
	}

	public final boolean isVisible() {
		return this.visibility.getAsBoolean();
	}

	public final S visibleWhen(BooleanSupplier condition) {
		this.visibility = condition;
		return self();
	}

	public final S onChange(Consumer<? super T> listener) {
		this.listeners.add(listener);
		return self();
	}

	public abstract T value();

	public abstract T defaultValue();

	public abstract void setValue(T value);

	public abstract Codec<T> codec();

	public final void reset() {
		setValue(defaultValue());
	}

	public final boolean isDefault() {
		return value().equals(defaultValue());
	}

	final void changed(T value) {
		for (Consumer<? super T> listener : this.listeners) {
			listener.accept(value);
		}
	}

	@SuppressWarnings("unchecked")
	private S self() {
		return (S) this;
	}
}
