package sh.rikka.client.render.skin;

import com.mojang.renderpearl.api.pipeline.ColorTargetState;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import java.util.Map;
import java.util.Set;
import net.minecraft.client.renderer.ShaderDefines;

public enum SkinFaces {
	ALL("", ShaderDefines.EMPTY),
	BACK("back/", new ShaderDefines(Map.of(), Set.of("SKIN_BACK_FACES"))),
	FRONT("front/", new ShaderDefines(Map.of(), Set.of("SKIN_FRONT_FACES")));

	private final String pathSegment;
	private final ShaderDefines defines;

	SkinFaces(String pathSegment, ShaderDefines defines) {
		this.pathSegment = pathSegment;
		this.defines = defines;
	}

	String pathSegment() {
		return this.pathSegment;
	}

	ShaderDefines defines() {
		return this.defines;
	}

	boolean appliesTo(RenderPipeline pipeline) {
		return this == ALL || isLayered(pipeline);
	}

	public static boolean isLayered(RenderPipeline pipeline) {
		if (pipeline.isCull()) {
			return false;
		}
		for (ColorTargetState target : pipeline.getColorTargetStates()) {
			if (target != null && target.blendFunction().isPresent()) {
				return true;
			}
		}
		return false;
	}
}
