package sh.rikka.client.mixin;

import com.mojang.renderpearl.backend.vulkan.VulkanFeatureSets;
import com.mojang.renderpearl.backend.vulkan.init.FeatureSet;
import java.util.Set;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import sh.rikka.client.ui.RikkaVulkan;

@Mixin(VulkanFeatureSets.class)
abstract class VulkanFeatureSetsMixin {
	@Inject(method = "requiredFeatureSets", at = @At("RETURN"))
	private static void rikka$requireRikkaFeatures(CallbackInfoReturnable<Set<FeatureSet>> callback) {
		callback.getReturnValue().add(RikkaVulkan.FEATURES);
	}
}
