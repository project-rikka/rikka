package sh.rikka.bench.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.renderpearl.api.commands.CommandEncoder;
import com.mojang.renderpearl.api.device.GpuSurface;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.bench.BenchMod;
import sh.rikka.bench.GpuSpan;
import sh.rikka.bench.Wait;

@Mixin(Minecraft.class)
abstract class MinecraftMixin {
	@Inject(method = "runTick", at = @At("TAIL"))
	private void bench$frame(CallbackInfo callback) {
		BenchMod.frame();
	}

	@Inject(method = "tick", at = @At("HEAD"))
	private void bench$tickStarting(CallbackInfo callback) {
		BenchMod.tickStarting();
	}

	@Inject(method = "tick", at = @At("TAIL"))
	private void bench$tick(CallbackInfo callback) {
		BenchMod.tick();
	}

	@Inject(method = "renderFrame", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/GameRenderer;render()V"))
	private void bench$renderStarting(CallbackInfo callback) {
		BenchMod.gpuBegin(GpuSpan.FRAME, null);
	}

	@Inject(method = "renderFrame", at = @At(value = "INVOKE", target = "Lcom/mojang/renderpearl/api/commands/CommandEncoder;submit()V"))
	private void bench$renderSubmitting(CallbackInfo callback) {
		BenchMod.gpuEnd(GpuSpan.FRAME, null);
	}

	@WrapOperation(
		method = "renderFrame",
		at = @At(value = "INVOKE", target = "Lcom/mojang/renderpearl/api/device/GpuSurface;acquireNextTexture()V")
	)
	private void bench$acquire(GpuSurface surface, Operation<Void> original) {
		long started = System.nanoTime();
		original.call(surface);
		BenchMod.waited(Wait.ACQUIRE, System.nanoTime() - started);
	}

	@WrapOperation(
		method = "renderFrame",
		at = @At(value = "INVOKE", target = "Lcom/mojang/renderpearl/api/commands/CommandEncoder;submit()V")
	)
	private void bench$submit(CommandEncoder encoder, Operation<Void> original) {
		long started = System.nanoTime();
		original.call(encoder);
		BenchMod.waited(Wait.SUBMIT, System.nanoTime() - started);
	}

	@WrapOperation(
		method = "renderFrame",
		at = @At(value = "INVOKE", target = "Lcom/mojang/renderpearl/api/device/GpuSurface;present()V")
	)
	private void bench$present(GpuSurface surface, Operation<Void> original) {
		long started = System.nanoTime();
		original.call(surface);
		BenchMod.waited(Wait.PRESENT, System.nanoTime() - started);
	}
}
