package sh.rikka.client.mixin.terrain;

import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.simulation.thread.WorkerPriority;

@Mixin(SectionRenderDispatcher.class)
abstract class SectionRenderDispatcherMixin {
	@Inject(method = "runTask", at = @At("HEAD"))
	private void rikka$lowerWorkerPriority(CallbackInfo callback) {
		WorkerPriority.lowerCurrentThread();
	}
}
