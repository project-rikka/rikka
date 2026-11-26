package sh.rikka.client.mixin.terrain;

import net.minecraft.client.renderer.fog.FogRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(FogRenderer.class)
public interface FogRendererAccessor {
	@Accessor("fogEnabled")
	static boolean rikka$fogEnabled() {
		throw new AssertionError();
	}
}
