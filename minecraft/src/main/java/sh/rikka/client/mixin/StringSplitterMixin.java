package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.StringSplitter;
import net.minecraft.network.chat.FormattedText;
import net.minecraft.network.chat.Style;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.util.FormattedCharSink;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import sh.rikka.client.mods.privacy.PrivacyHooks;

@Mixin(StringSplitter.class)
abstract class StringSplitterMixin {
	@WrapOperation(
		method = "stringWidth(Ljava/lang/String;)F",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/util/StringDecomposer;iterateFormatted(Ljava/lang/String;Lnet/minecraft/network/chat/Style;Lnet/minecraft/util/FormattedCharSink;)Z"
		)
	)
	private boolean rikka$protectString(String text, Style style, FormattedCharSink output, Operation<Boolean> original) {
		if (!PrivacyHooks.protectsText()) {
			return original.call(text, style, output);
		}
		return PrivacyHooks.text(sink -> original.call(text, style, sink)).accept(output);
	}

	@WrapOperation(
		method = "stringWidth(Lnet/minecraft/network/chat/FormattedText;)F",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/util/StringDecomposer;iterateFormatted(Lnet/minecraft/network/chat/FormattedText;Lnet/minecraft/network/chat/Style;Lnet/minecraft/util/FormattedCharSink;)Z"
		)
	)
	private boolean rikka$protectText(FormattedText text, Style style, FormattedCharSink output, Operation<Boolean> original) {
		if (!PrivacyHooks.protectsText()) {
			return original.call(text, style, output);
		}
		return PrivacyHooks.text(sink -> original.call(text, style, sink)).accept(output);
	}

	@ModifyVariable(method = "stringWidth(Lnet/minecraft/util/FormattedCharSequence;)F", at = @At("HEAD"), argsOnly = true)
	private FormattedCharSequence rikka$protectSequence(FormattedCharSequence text) {
		return PrivacyHooks.text(text);
	}
}
