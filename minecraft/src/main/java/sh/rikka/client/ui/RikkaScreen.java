package sh.rikka.client.ui;

import java.nio.charset.StandardCharsets;
import java.util.Optional;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.client.input.PreeditEvent;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import os.rikka.Context;
import os.rikka.EmbeddedSurface;
import os.rikka.EmbeddedSurfaceBuilder;
import os.rikka.KeyCode;
import os.rikka.PointerButton;
import os.rikka.Rect;
import os.rikka.Rikka;
import os.rikka.View;
import os.rikka.VulkanImage;

public abstract class RikkaScreen extends Screen {
	private @Nullable EmbeddedSurface surface;
	private @Nullable SurfaceGeometry geometry;
	private boolean editing;

	protected RikkaScreen(Component title) {
		super(title);
	}

	protected abstract void view(View view);

	protected void opened(Context context) {
	}

	protected final void refresh() {
		EmbeddedSurface shown = this.surface;
		if (shown != null) {
			shown.getWindow().invalidate();
		}
	}

	@Override
	public void added() {
		super.added();
		this.surface = RikkaUi.openSurface(
			new EmbeddedSurfaceBuilder(this::view)
				.onStart(this::opened)
				.onCloseRequest(context -> this.onClose())
		);
	}

	@Override
	public void removed() {
		super.removed();
		this.minecraft.textInputManager().stopTextInput(this);
		this.editing = false;
		this.geometry = null;
		EmbeddedSurface shown = this.surface;
		this.surface = null;
		if (shown != null) {
			RikkaUi.closeSurface(shown);
		}
	}

	final void draw(VulkanImage target, SurfaceGeometry geometry) {
		EmbeddedSurface shown = this.surface;
		if (shown == null) {
			return;
		}
		if (!geometry.equals(this.geometry)) {
			this.geometry = geometry;
			shown.resize(geometry.width(), geometry.height(), geometry.scale());
		}
		shown.render(target);
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
		EmbeddedSurface shown = this.surface;
		SurfaceGeometry placed = this.geometry;
		if (shown == null || placed == null) {
			return;
		}
		graphics.requestCursor(RikkaInput.cursor(shown.getCursor()));
		followCaret(shown.getCaret(), placed);
	}

	@Override
	public void extractBackground(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
		if (this.minecraft.level == null) {
			this.extractPanorama(graphics, a);
		}
	}

	@Override
	public void mouseMoved(double x, double y) {
		EmbeddedSurface shown = this.surface;
		SurfaceGeometry placed = this.geometry;
		if (shown != null && placed != null) {
			shown.pointerMove(placed.fromGui(x), placed.fromGui(y));
		}
	}

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		EmbeddedSurface shown = this.surface;
		SurfaceGeometry placed = this.geometry;
		Optional<PointerButton> button = RikkaInput.button(event.button());
		if (shown == null || placed == null || button.isEmpty()) {
			return false;
		}
		return shown.pointerPress(
			placed.fromGui(event.x()),
			placed.fromGui(event.y()),
			button.get(),
			RikkaInput.modifiers(event.modifiers())
		);
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		EmbeddedSurface shown = this.surface;
		SurfaceGeometry placed = this.geometry;
		Optional<PointerButton> button = RikkaInput.button(event.button());
		if (shown == null || placed == null || button.isEmpty()) {
			return false;
		}
		return shown.pointerRelease(
			placed.fromGui(event.x()),
			placed.fromGui(event.y()),
			button.get(),
			RikkaInput.modifiers(event.modifiers())
		);
	}

	@Override
	public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
		EmbeddedSurface shown = this.surface;
		SurfaceGeometry placed = this.geometry;
		if (shown == null || placed == null) {
			return false;
		}
		return shown.scroll(
			placed.fromGui(x),
			placed.fromGui(y),
			(float) scrollX,
			(float) scrollY,
			RikkaInput.heldModifiers()
		);
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		EmbeddedSurface shown = this.surface;
		if (shown != null) {
			if (this.editing && RikkaInput.typesText(event)) {
				return true;
			}
			Optional<KeyCode> key = RikkaInput.key(event);
			if (key.isPresent() && shown.keyPress(key.get(), RikkaInput.modifiers(event.modifiers()))) {
				return true;
			}
		}
		return super.keyPressed(event);
	}

	@Override
	public boolean keyReleased(KeyEvent event) {
		EmbeddedSurface shown = this.surface;
		if (shown == null || (this.editing && RikkaInput.typesText(event))) {
			return false;
		}
		Optional<KeyCode> key = RikkaInput.key(event);
		return key.isPresent() && shown.keyRelease(key.get(), RikkaInput.modifiers(event.modifiers()));
	}

	@Override
	public boolean charTyped(CharacterEvent event) {
		EmbeddedSurface shown = this.surface;
		return shown != null && shown.typeText(event.codepointAsString());
	}

	@Override
	public boolean preeditUpdated(@Nullable PreeditEvent event) {
		EmbeddedSurface shown = this.surface;
		if (shown == null) {
			return false;
		}
		if (event == null) {
			return shown.composeText("", Rikka.NONE, Rikka.NONE);
		}
		String text = event.fullText();
		int caret = text.substring(0, event.caretPosition()).getBytes(StandardCharsets.UTF_8).length;
		return shown.composeText(text, caret, caret);
	}

	private void followCaret(Rect caret, SurfaceGeometry placed) {
		boolean editing = caret.width() > 0.0F || caret.height() > 0.0F;
		if (editing) {
			this.minecraft.textInputManager().setTextInputArea(
				placed.toGui(caret.x()),
				placed.toGui(caret.y()),
				placed.toGui(caret.x() + caret.width()),
				placed.toGui(caret.y() + caret.height())
			);
			this.minecraft.textInputManager().startTextInput(this);
		} else if (this.editing) {
			this.minecraft.textInputManager().stopTextInput(this);
		}
		this.editing = editing;
	}
}
