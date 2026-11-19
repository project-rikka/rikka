package sh.rikka.client.setting;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.EnumSet;
import java.util.List;
import java.util.Optional;
import java.util.regex.Pattern;
import net.minecraft.core.Registry;

public final class Settings {
	private static final Pattern ID = Pattern.compile("[a-z0-9_]+");

	private final String translationPrefix;
	private final List<Setting<?, ?>> settings = new ArrayList<>();
	private final List<Setting<?, ?>> view = Collections.unmodifiableList(this.settings);
	private boolean locked;

	public Settings(String translationPrefix) {
		this.translationPrefix = translationPrefix;
	}

	public static boolean isValidId(String id) {
		return ID.matcher(id).matches();
	}

	public List<Setting<?, ?>> all() {
		return this.view;
	}

	public Optional<Setting<?, ?>> find(String id) {
		for (Setting<?, ?> setting : this.settings) {
			if (setting.id().equals(id)) {
				return Optional.of(setting);
			}
		}
		return Optional.empty();
	}

	public void lock() {
		this.locked = true;
	}

	public BoolSetting bool(String id, boolean defaultValue) {
		return add(new BoolSetting(id, key(id), defaultValue));
	}

	public IntSetting integer(String id, int defaultValue, int min, int max) {
		return add(new IntSetting(id, key(id), defaultValue, min, max));
	}

	public DoubleSetting number(String id, double defaultValue, double min, double max) {
		return add(new DoubleSetting(id, key(id), defaultValue, min, max));
	}

	public ColorSetting color(String id, int defaultArgb) {
		return add(new ColorSetting(id, key(id), defaultArgb));
	}

	public StringSetting text(String id, String defaultValue) {
		return add(new StringSetting(id, key(id), defaultValue));
	}

	public <E extends Enum<E>> EnumSetting<E> choice(String id, E defaultValue) {
		return add(new EnumSetting<>(id, key(id), defaultValue));
	}

	public KeybindSetting keybind(String id) {
		return keybind(id, InputConstants.UNKNOWN);
	}

	public KeybindSetting keybind(String id, int keyboardKey) {
		return keybind(id, InputConstants.Type.KEYBOARD.getOrCreate(keyboardKey));
	}

	public KeybindSetting keybind(String id, InputConstants.Key defaultKey) {
		return add(new KeybindSetting(id, key(id), defaultKey));
	}

	public IntRangeSetting integerRange(String id, int low, int high, int min, int max) {
		return add(new IntRangeSetting(id, key(id), new IntRange(low, high), min, max));
	}

	public DoubleRangeSetting numberRange(String id, double low, double high, double min, double max) {
		return add(new DoubleRangeSetting(id, key(id), new DoubleRange(low, high), min, max));
	}

	public <E extends Enum<E>> EnumSetSetting<E> choices(String id, EnumSet<E> defaults) {
		return add(new EnumSetSetting<>(id, key(id), defaults));
	}

	public <T> RegistrySetSetting<T> registrySet(String id, Registry<T> registry, Collection<T> defaults) {
		return add(new RegistrySetSetting<>(id, key(id), registry, defaults));
	}

	public StringListSetting textList(String id, String... defaults) {
		return add(new StringListSetting(id, key(id), List.of(defaults)));
	}

	public PositionSetting position(String id, ScreenPosition defaultValue) {
		return add(new PositionSetting(id, key(id), defaultValue));
	}

	private String key(String id) {
		if (!isValidId(id)) {
			throw new IllegalArgumentException("invalid setting id: " + id);
		}
		return this.translationPrefix + "." + id;
	}

	private <S extends Setting<?, ?>> S add(S setting) {
		if (this.locked) {
			throw new IllegalStateException("settings are locked; declare " + setting.id() + " before registration");
		}
		if (find(setting.id()).isPresent()) {
			throw new IllegalArgumentException("duplicate setting id: " + setting.id());
		}
		this.settings.add(setting);
		return setting;
	}
}
