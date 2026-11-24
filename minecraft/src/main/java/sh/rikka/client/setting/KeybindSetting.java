package sh.rikka.client.setting;

import com.mojang.blaze3d.platform.InputConstants;
import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import net.minecraft.client.Minecraft;
import org.lwjgl.sdl.SDLMouse;

public final class KeybindSetting extends Setting<InputConstants.Key, KeybindSetting> {
	private static final Codec<InputConstants.Key> CODEC = Codec.STRING.comapFlatMap(KeybindSetting::parse, InputConstants.Key::getName);

	private final InputConstants.Key defaultValue;
	private volatile InputConstants.Key value;

	KeybindSetting(String id, String translationKey, InputConstants.Key defaultValue) {
		super(id, translationKey);
		this.defaultValue = defaultValue;
		this.value = defaultValue;
	}

	private static DataResult<InputConstants.Key> parse(String name) {
		try {
			return DataResult.success(InputConstants.getKey(name));
		} catch (IllegalArgumentException | IndexOutOfBoundsException failure) {
			return DataResult.error(() -> "unknown key: " + name);
		}
	}

	public InputConstants.Key get() {
		return this.value;
	}

	public void set(InputConstants.Key value) {
		if (this.value.equals(value)) {
			return;
		}
		this.value = value;
		changed(value);
	}

	public void unbind() {
		set(InputConstants.UNKNOWN);
	}

	public boolean isBound() {
		return !this.value.equals(InputConstants.UNKNOWN);
	}

	public boolean matches(InputConstants.Key key) {
		return isBound() && this.value.equals(key);
	}

	public boolean isDown() {
		InputConstants.Key key = this.value;
		if (key.equals(InputConstants.UNKNOWN) || Minecraft.getInstance().gui.screen() != null) {
			return false;
		}
		return switch (key.getType()) {
			case KEYBOARD -> InputConstants.isKeyDown(key.getValue());
			case MOUSE -> (SDLMouse.SDL_GetMouseState(null, null) & buttonMask(key.getValue())) != 0;
		};
	}

	private static int buttonMask(int button) {
		return 1 << (button - 1);
	}

	@Override
	public InputConstants.Key value() {
		return this.value;
	}

	@Override
	public InputConstants.Key defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(InputConstants.Key value) {
		set(value);
	}

	@Override
	public Codec<InputConstants.Key> codec() {
		return CODEC;
	}
}
