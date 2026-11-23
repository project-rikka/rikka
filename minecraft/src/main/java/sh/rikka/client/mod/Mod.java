package sh.rikka.client.mod;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.Collection;
import java.util.EnumSet;
import net.minecraft.core.Registry;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.event.EventBus;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.DoubleRangeSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.IntRangeSetting;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.KeybindSetting;
import sh.rikka.client.setting.PositionSetting;
import sh.rikka.client.setting.RegistrySetSetting;
import sh.rikka.client.setting.ScreenPosition;
import sh.rikka.client.setting.Settings;
import sh.rikka.client.setting.StringListSetting;
import sh.rikka.client.setting.StringSetting;

public abstract class Mod {
	private static final Logger LOGGER = LoggerFactory.getLogger(Mod.class);
	private static final String TRANSLATION_PREFIX = RikkaClient.MOD_ID + ".mod.";

	private final String id;
	private final Category category;
	private final String icon;
	private final Settings settings;
	private final Settings controls;
	private final KeybindSetting bind;
	private final EnumSetting<BindMode> bindMode;
	private volatile boolean enabled;
	private @Nullable EventBus bus;

	protected Mod(String id, Category category, String icon) {
		if (!Settings.isValidId(id)) {
			throw new IllegalArgumentException("invalid mod id: " + id);
		}
		this.id = id;
		this.category = category;
		this.icon = icon;
		this.settings = new Settings(TRANSLATION_PREFIX + id + ".setting");
		this.controls = new Settings(RikkaClient.MOD_ID + ".setting");
		this.bind = this.controls.keybind("bind");
		this.bindMode = this.controls.choice("bind_mode", BindMode.TOGGLE);
		this.controls.lock();
	}

	public final String id() {
		return this.id;
	}

	public final Category category() {
		return this.category;
	}

	public final String icon() {
		return this.icon;
	}

	public final String translationKey() {
		return TRANSLATION_PREFIX + this.id;
	}

	public final Component name() {
		return Component.translatable(translationKey());
	}

	public final Component description() {
		return Component.translatable(translationKey() + ".description");
	}

	public final Settings settings() {
		return this.settings;
	}

	public final Settings controls() {
		return this.controls;
	}

	public final KeybindSetting bind() {
		return this.bind;
	}

	public final EnumSetting<BindMode> bindMode() {
		return this.bindMode;
	}

	public final boolean isEnabled() {
		return this.enabled;
	}

	public final void toggle() {
		setEnabled(!this.enabled);
	}

	public final void setEnabled(boolean enabled) {
		EventBus bus = this.bus;
		if (bus == null) {
			throw new IllegalStateException("mod " + this.id + " is not registered");
		}
		if (this.enabled == enabled) {
			return;
		}
		if (enabled) {
			enable(bus);
		} else {
			disable(bus);
		}
	}

	private void enable(EventBus bus) {
		bus.subscribe(this);
		this.enabled = true;
		try {
			onEnable();
		} catch (RuntimeException failure) {
			LOGGER.error("failed to enable {}", this.id, failure);
			if (this.enabled) {
				this.enabled = false;
				bus.unsubscribe(this);
			}
			return;
		}
		if (this.enabled) {
			bus.post(new ModToggleEvent(this, true));
		}
	}

	private void disable(EventBus bus) {
		this.enabled = false;
		try {
			onDisable();
		} catch (RuntimeException failure) {
			LOGGER.error("failed to disable {}", this.id, failure);
		} finally {
			bus.unsubscribe(this);
		}
		bus.post(new ModToggleEvent(this, false));
	}

	protected void onEnable() {
	}

	protected void onDisable() {
	}

	final void attach(EventBus bus) {
		if (this.bus != null) {
			throw new IllegalStateException("mod " + this.id + " is already registered");
		}
		this.bus = bus;
	}

	protected final BoolSetting bool(String id, boolean defaultValue) {
		return this.settings.bool(id, defaultValue);
	}

	protected final IntSetting integer(String id, int defaultValue, int min, int max) {
		return this.settings.integer(id, defaultValue, min, max);
	}

	protected final DoubleSetting number(String id, double defaultValue, double min, double max) {
		return this.settings.number(id, defaultValue, min, max);
	}

	protected final ColorSetting color(String id, int defaultArgb) {
		return this.settings.color(id, defaultArgb);
	}

	protected final StringSetting text(String id, String defaultValue) {
		return this.settings.text(id, defaultValue);
	}

	protected final <E extends Enum<E>> EnumSetting<E> choice(String id, E defaultValue) {
		return this.settings.choice(id, defaultValue);
	}

	protected final KeybindSetting keybind(String id) {
		return this.settings.keybind(id);
	}

	protected final KeybindSetting keybind(String id, int keyboardKey) {
		return this.settings.keybind(id, keyboardKey);
	}

	protected final KeybindSetting keybind(String id, InputConstants.Key defaultKey) {
		return this.settings.keybind(id, defaultKey);
	}

	protected final IntRangeSetting integerRange(String id, int low, int high, int min, int max) {
		return this.settings.integerRange(id, low, high, min, max);
	}

	protected final DoubleRangeSetting numberRange(String id, double low, double high, double min, double max) {
		return this.settings.numberRange(id, low, high, min, max);
	}

	protected final <E extends Enum<E>> EnumSetSetting<E> choices(String id, EnumSet<E> defaults) {
		return this.settings.choices(id, defaults);
	}

	protected final <T> RegistrySetSetting<T> registrySet(String id, Registry<T> registry, Collection<T> defaults) {
		return this.settings.registrySet(id, registry, defaults);
	}

	protected final StringListSetting textList(String id, String... defaults) {
		return this.settings.textList(id, defaults);
	}

	protected final PositionSetting position(String id, ScreenPosition defaultValue) {
		return this.settings.position(id, defaultValue);
	}
}
