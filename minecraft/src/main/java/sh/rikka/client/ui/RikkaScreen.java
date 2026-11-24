package sh.rikka.client.ui;

import com.mojang.blaze3d.platform.Window;
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
	private boolean leaving;
	private @Nullable Screen next;

	protected RikkaScreen(Component title) {
		super(title);
	}

	protected abstract void view(View view);

	protected void opened(Context context) {
	}

	protected boolean pressed(float x, float y, MouseButtonEvent event) {
		return false;
	}

	protected void moved(float x, float y, boolean used) {
	}

	protected boolean released(float x, float y, MouseButtonEvent event) {
		return false;
	}

	protected boolean scrolled(float x, float y, double amount) {
		return false;
	}

	protected final void refresh() {
		EmbeddedSurface shown = this.surface;
		if (shown != null) {
			shown.getWindow().invalidate();
		}
	}

	protected final float surfaceWidth() {
		return measured().width();
	}

	protected final float surfaceHeight() {
		return measured().height();
	}

	protected final float hudUnit() {
		return measured().hudUnit();
	}

	protected final boolean isLeaving() {
		return this.leaving;
	}

	protected final void leaveTo(@Nullable Screen next) {
		EmbeddedSurface shown = this.surface;
		if (shown == null) {
			this.minecraft.gui.setScreen(next);
			return;
		}
		if (this.leaving) {
			return;
		}
		this.leaving = true;
		this.next = next;
		this.minecraft.textInputManager().stopTextInput(this);
		this.editing = false;
		shown.leave();
	}

	@Override
	public void onClose() {
		leaveTo(null);
	}

	@Override
	public void added() {
		super.added();
		this.leaving = false;
		this.next = null;
		this.surface = RikkaUi.openSurface(
			new EmbeddedSurfaceBuilder(this::view)
				.onStart(this::opened)
				.onCloseRequest(context -> this.onClose())
				.onLeft(context -> this.minecraft.schedule(this::left))
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

	final void draw(VulkanImage target, Window window) {
		EmbeddedSurface shown = this.surface;
		if (shown == null) {
			return;
		}
		SurfaceGeometry measured = SurfaceGeometry.of(window, target, SurfaceScale.DISPLAY);
		if (!measured.equals(this.geometry)) {
			this.geometry = measured;
			shown.resize(measured.width(), measured.height(), measured.scale());
			shown.getWindow().invalidate();
		}
		shown.render(target);
	}

	private void left() {
		if (this.leaving && this.minecraft.gui.screen() == this) {
			this.minecraft.gui.setScreen(this.next);
		}
	}

	private SurfaceGeometry measured() {
		SurfaceGeometry placed = this.geometry;
		if (placed == null) {
			throw new IllegalStateException(getClass().getSimpleName() + " is not drawn yet");
		}
		return placed;
	}

	private @Nullable EmbeddedSurface interactive() {
		return this.leaving || this.geometry == null ? null : this.surface;
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
		EmbeddedSurface shown = interactive();
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
		EmbeddedSurface shown = interactive();
		SurfaceGeometry placed = this.geometry;
		if (shown == null || placed == null) {
			return;
		}
		float surfaceX = placed.fromGui(x);
		float surfaceY = placed.fromGui(y);
		boolean used = shown.pointerMove(surfaceX, surfaceY);
		moved(surfaceX, surfaceY, used);
	}

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		EmbeddedSurface shown = interactive();
		SurfaceGeometry placed = this.geometry;
		if (shown == null || placed == null) {
			return this.leaving;
		}
		float x = placed.fromGui(event.x());
		float y = placed.fromGui(event.y());
		Optional<PointerButton> button = RikkaInput.button(event.button());
		boolean used = button.isPresent() && shown.pointerPress(x, y, button.get(), RikkaInput.modifiers(event.modifiers()));
		return used || pressed(x, y, event);
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		EmbeddedSurface shown = interactive();
		SurfaceGeometry placed = this.geometry;
		if (shown == null || placed == null) {
			return this.leaving;
		}
		float x = placed.fromGui(event.x());
		float y = placed.fromGui(event.y());
		Optional<PointerButton> button = RikkaInput.button(event.button());
		boolean used = button.isPresent() && shown.pointerRelease(x, y, button.get(), RikkaInput.modifiers(event.modifiers()));
		return released(x, y, event) || used;
	}

	@Override
	public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
		EmbeddedSurface shown = interactive();
		SurfaceGeometry placed = this.geometry;
		if (shown == null || placed == null) {
			return this.leaving;
		}
		float surfaceX = placed.fromGui(x);
		float surfaceY = placed.fromGui(y);
		boolean used = shown.scroll(surfaceX, surfaceY, (float) scrollX, (float) scrollY, RikkaInput.heldModifiers());
		return used || scrolled(surfaceX, surfaceY, scrollY);
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		if (this.leaving) {
			return true;
		}
		EmbeddedSurface shown = interactive();
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
		EmbeddedSurface shown = interactive();
		if (shown == null || (this.editing && RikkaInput.typesText(event))) {
			return false;
		}
		Optional<KeyCode> key = RikkaInput.key(event);
		return key.isPresent() && shown.keyRelease(key.get(), RikkaInput.modifiers(event.modifiers()));
	}

	@Override
	public boolean charTyped(CharacterEvent event) {
		EmbeddedSurface shown = interactive();
		return shown != null && shown.typeText(event.codepointAsString());
	}

	@Override
	public boolean preeditUpdated(@Nullable PreeditEvent event) {
		EmbeddedSurface shown = interactive();
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
