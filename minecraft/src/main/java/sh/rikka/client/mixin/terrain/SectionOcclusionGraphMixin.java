package sh.rikka.client.mixin.terrain;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.blaze3d.systems.RenderSystem;
import java.util.Queue;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.atomic.AtomicReference;
import net.minecraft.client.renderer.SectionOcclusionGraph;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(SectionOcclusionGraph.class)
abstract class SectionOcclusionGraphMixin {
	@Unique
	private static final long PARTIAL_UPDATE_NANOS = 1_000_000L;

	@Shadow
	@Final
	private AtomicReference<?> currentGraph;

	@Unique
	private long rikka$partialUpdateDeadline;

	@Inject(method = "runPartialUpdate", at = @At("HEAD"))
	private void rikka$startPartialUpdate(CallbackInfo callback) {
		this.rikka$partialUpdateDeadline = System.nanoTime() + PARTIAL_UPDATE_NANOS;
	}

	@WrapOperation(method = "runUpdates", at = @At(value = "INVOKE", target = "Ljava/util/Queue;isEmpty()Z"))
	private boolean rikka$sliceUpdates(Queue<SectionOcclusionGraph.Node> queue, Operation<Boolean> original) {
		if (original.call(queue)) {
			return true;
		}
		if (!RenderSystem.isOnRenderThread() || System.nanoTime() < this.rikka$partialUpdateDeadline) {
			return false;
		}
		BlockingQueue<SectionRenderDispatcher.RenderSection> resumed = ((OcclusionGraphStateAccessor) this.currentGraph.get()).rikka$sectionsToPropagateFrom();
		for (SectionOcclusionGraph.Node node : queue) {
			resumed.add(((OcclusionGraphNodeAccessor) node).rikka$section());
		}
		queue.clear();
		return true;
	}
}
