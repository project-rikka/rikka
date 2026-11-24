package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.gui.Font;
import net.minecraft.network.chat.Style;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.util.FormattedCharSink;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import sh.rikka.client.mods.privacy.PrivacyHooks;

@Mixin(Font.class)
abstract class FontMixin {
	@WrapOperation(
		method = "prepareText(Ljava/lang/String;FFIZI)Lnet/minecraft/client/gui/Font$PreparedText;",
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

	@ModifyVariable(
		method = "prepareText(Lnet/minecraft/util/FormattedCharSequence;FFIZZI)Lnet/minecraft/client/gui/Font$PreparedText;",
		at = @At("HEAD"),
		argsOnly = true
	)
	private FormattedCharSequence rikka$protectSequence(FormattedCharSequence text) {
		return PrivacyHooks.text(text);
	}

	@ModifyVariable(method = "prepare8xTextOutline", at = @At("HEAD"), argsOnly = true)
	private FormattedCharSequence rikka$protectOutline(FormattedCharSequence text) {
		return PrivacyHooks.text(text);
	}
}
