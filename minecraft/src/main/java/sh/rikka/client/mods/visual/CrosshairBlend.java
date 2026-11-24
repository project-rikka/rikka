package sh.rikka.client.mods.visual;

import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import net.minecraft.client.renderer.RenderPipelines;

public enum CrosshairBlend {
	NORMAL,
	INVERT;

	RenderPipeline pipeline() {
		return switch (this) {
			case NORMAL -> RenderPipelines.GUI;
			case INVERT -> RenderPipelines.GUI_INVERT;
		};
	}
}
