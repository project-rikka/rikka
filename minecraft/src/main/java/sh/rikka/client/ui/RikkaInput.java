package sh.rikka.client.ui;

import com.mojang.blaze3d.platform.InputConstants;
import com.mojang.blaze3d.platform.cursor.CursorType;
import com.mojang.blaze3d.platform.cursor.CursorTypes;
import java.util.EnumSet;
import java.util.Optional;
import java.util.Set;
import net.minecraft.client.input.KeyEvent;
import org.jspecify.annotations.Nullable;
import org.lwjgl.sdl.SDLKeyboard;
import os.rikka.CursorShape;
import os.rikka.KeyCode;
import os.rikka.Modifier;
import os.rikka.PointerButton;

final class RikkaInput {
	private static final int SDL_KEYCODE_SCANCODE_MASK = 1 << 30;
	private static final int SDL_SCANCODE_APPLICATION = 101;
	private static final int SDL_SYSTEM_CURSOR_WAIT = 2;
	private static final int SDL_SYSTEM_CURSOR_PROGRESS = 4;
	private static final int SDL_SYSTEM_CURSOR_NWSE_RESIZE = 5;
	private static final int SDL_SYSTEM_CURSOR_NESW_RESIZE = 6;
	private static final CursorType WAIT = CursorType.createStandardCursor(SDL_SYSTEM_CURSOR_WAIT, "wait", CursorTypes.ARROW);
	private static final CursorType PROGRESS = CursorType.createStandardCursor(SDL_SYSTEM_CURSOR_PROGRESS, "progress", CursorTypes.ARROW);
	private static final CursorType RESIZE_NWSE = CursorType.createStandardCursor(SDL_SYSTEM_CURSOR_NWSE_RESIZE, "resize_nwse", CursorTypes.ARROW);
	private static final CursorType RESIZE_NESW = CursorType.createStandardCursor(SDL_SYSTEM_CURSOR_NESW_RESIZE, "resize_nesw", CursorTypes.ARROW);

	private RikkaInput() {
	}

	static Set<Modifier> modifiers(int held) {
		Set<Modifier> modifiers = EnumSet.noneOf(Modifier.class);
		if ((held & InputConstants.MOD_SHIFT) != 0) {
			modifiers.add(Modifier.SHIFT);
		}
		if ((held & InputConstants.MOD_CONTROL) != 0) {
			modifiers.add(Modifier.CONTROL);
		}
		if ((held & InputConstants.MOD_ALT) != 0) {
			modifiers.add(Modifier.ALT);
		}
		if ((held & InputConstants.MOD_SUPER) != 0) {
			modifiers.add(Modifier.LOGO);
		}
		return modifiers;
	}

	static Set<Modifier> heldModifiers() {
		return modifiers(SDLKeyboard.SDL_GetModState());
	}

	static boolean typesText(KeyEvent event) {
		boolean chord = (event.modifiers() & (InputConstants.MOD_CONTROL | InputConstants.MOD_ALT | InputConstants.MOD_SUPER)) != 0;
		return !chord && (event.key() == InputConstants.KEY_SPACE || character(event).isPresent());
	}

	static Optional<KeyCode> key(KeyEvent event) {
		@Nullable KeyCode named = switch (event.key()) {
			case InputConstants.KEY_RETURN, InputConstants.KEY_NUMPADENTER -> KeyCode.ENTER;
			case InputConstants.KEY_ESCAPE -> KeyCode.ESCAPE;
			case InputConstants.KEY_TAB -> KeyCode.TAB;
			case InputConstants.KEY_BACKSPACE -> KeyCode.BACKSPACE;
			case InputConstants.KEY_DELETE -> KeyCode.DELETE;
			case InputConstants.KEY_INSERT -> KeyCode.INSERT;
			case InputConstants.KEY_SPACE -> KeyCode.SPACE;
			case InputConstants.KEY_UP -> KeyCode.ARROW_UP;
			case InputConstants.KEY_DOWN -> KeyCode.ARROW_DOWN;
			case InputConstants.KEY_LEFT -> KeyCode.ARROW_LEFT;
			case InputConstants.KEY_RIGHT -> KeyCode.ARROW_RIGHT;
			case InputConstants.KEY_HOME -> KeyCode.HOME;
			case InputConstants.KEY_END -> KeyCode.END;
			case InputConstants.KEY_PAGEUP -> KeyCode.PAGE_UP;
			case InputConstants.KEY_PAGEDOWN -> KeyCode.PAGE_DOWN;
			case SDL_SCANCODE_APPLICATION -> KeyCode.MENU;
			default -> functionKey(event.key());
		};
		return named != null ? Optional.of(named) : character(event).map(KeyCode::new);
	}

	private static @Nullable KeyCode functionKey(int scancode) {
		if (scancode >= InputConstants.KEY_F1 && scancode <= InputConstants.KEY_F12) {
			return new KeyCode(KeyCode.F1.value() + scancode - InputConstants.KEY_F1);
		}
		if (scancode >= InputConstants.KEY_F13 && scancode <= InputConstants.KEY_F24) {
			return new KeyCode(KeyCode.F13.value() + scancode - InputConstants.KEY_F13);
		}
		return null;
	}

	private static Optional<Integer> character(KeyEvent event) {
		int keycode = event.keycode();
		boolean typed = (keycode & SDL_KEYCODE_SCANCODE_MASK) == 0
			&& Character.isValidCodePoint(keycode)
			&& !Character.isISOControl(keycode)
			&& keycode != ' ';
		return typed ? Optional.of(keycode) : Optional.empty();
	}

	static Optional<PointerButton> button(int button) {
		return switch (button) {
			case InputConstants.MOUSE_BUTTON_LEFT -> Optional.of(PointerButton.LEFT);
			case InputConstants.MOUSE_BUTTON_RIGHT -> Optional.of(PointerButton.RIGHT);
			case InputConstants.MOUSE_BUTTON_MIDDLE -> Optional.of(PointerButton.MIDDLE);
			case InputConstants.MOUSE_BUTTON_4 -> Optional.of(PointerButton.BACK);
			case InputConstants.MOUSE_BUTTON_5 -> Optional.of(PointerButton.FORWARD);
			default -> Optional.empty();
		};
	}

	static CursorType cursor(CursorShape shape) {
		return switch (shape) {
			case DEFAULT, HIDDEN, HELP, ZOOM_IN, ZOOM_OUT -> CursorTypes.ARROW;
			case POINTER -> CursorTypes.POINTING_HAND;
			case TEXT -> CursorTypes.IBEAM;
			case WAIT -> WAIT;
			case PROGRESS -> PROGRESS;
			case CROSSHAIR -> CursorTypes.CROSSHAIR;
			case MOVE, GRAB, GRABBING -> CursorTypes.RESIZE_ALL;
			case NOT_ALLOWED -> CursorTypes.NOT_ALLOWED;
			case RESIZE_HORIZONTAL -> CursorTypes.RESIZE_EW;
			case RESIZE_VERTICAL -> CursorTypes.RESIZE_NS;
			case RESIZE_DIAGONAL_DOWN -> RESIZE_NWSE;
			case RESIZE_DIAGONAL_UP -> RESIZE_NESW;
		};
	}
}
