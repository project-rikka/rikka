package sh.rikka.client.event;

import java.lang.invoke.CallSite;
import java.lang.invoke.LambdaConversionException;
import java.lang.invoke.LambdaMetafactory;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

final class ListenerMethods {
	private static final MethodType HANDLE = MethodType.methodType(void.class, Event.class);
	private static final Comparator<Method> SIGNATURE_ORDER = Comparator.comparing(ListenerMethods::signature);
	private static final ClassValue<List<ListenerMethod>> METHODS = new ClassValue<>() {
		@Override
		protected List<ListenerMethod> computeValue(Class<?> type) {
			return discover(type);
		}
	};

	private ListenerMethods() {
	}

	static List<ListenerMethod> of(Class<?> type) {
		return METHODS.get(type);
	}

	private static List<ListenerMethod> discover(Class<?> type) {
		List<ListenerMethod> found = new ArrayList<>();
		Set<String> seen = new HashSet<>();
		for (Class<?> current = type; current != null && current != Object.class; current = current.getSuperclass()) {
			Method[] methods = current.getDeclaredMethods();
			Arrays.sort(methods, SIGNATURE_ORDER);
			for (Method method : methods) {
				if (method.isSynthetic() || method.isBridge()) {
					continue;
				}
				boolean overridden = seen.contains(overrideKey(method));
				seen.addAll(overrideKeys(method));
				EventHandler handler = method.getAnnotation(EventHandler.class);
				if (handler != null && !overridden) {
					found.add(create(method, handler));
				}
			}
		}
		return List.copyOf(found);
	}

	private static String signature(Method method) {
		return method.getName() + MethodType.methodType(method.getReturnType(), method.getParameterTypes()).toMethodDescriptorString();
	}

	private static String overrideKey(Method method) {
		int modifiers = method.getModifiers();
		if (Modifier.isPrivate(modifiers)) {
			return method.getDeclaringClass().getName() + '#' + signature(method);
		}
		if (Modifier.isPublic(modifiers) || Modifier.isProtected(modifiers)) {
			return signature(method);
		}
		return method.getDeclaringClass().getPackageName() + '#' + signature(method);
	}

	private static List<String> overrideKeys(Method method) {
		int modifiers = method.getModifiers();
		if (Modifier.isPublic(modifiers) || Modifier.isProtected(modifiers)) {
			return List.of(signature(method), method.getDeclaringClass().getPackageName() + '#' + signature(method));
		}
		return List.of(overrideKey(method));
	}

	private static ListenerMethod create(Method method, EventHandler handler) {
		String name = method.getDeclaringClass().getName() + "." + method.getName();
		int modifiers = method.getModifiers();
		if (Modifier.isStatic(modifiers) || Modifier.isAbstract(modifiers)) {
			throw new IllegalArgumentException("@EventHandler " + name + " must be a concrete instance method");
		}
		if (method.getReturnType() != void.class || method.getParameterCount() != 1) {
			throw new IllegalArgumentException("@EventHandler " + name + " must return void and take one event");
		}
		Class<?> parameter = method.getParameterTypes()[0];
		if (!Event.class.isAssignableFrom(parameter) || !Modifier.isFinal(parameter.getModifiers())) {
			throw new IllegalArgumentException("@EventHandler " + name + " must take a final Event type, not " + parameter.getName());
		}
		if (handler.receiveCancelled() && !CancellableEvent.class.isAssignableFrom(parameter)) {
			throw new IllegalArgumentException("@EventHandler " + name + " receives cancelled events of non-cancellable " + parameter.getName());
		}
		Class<?> owner = method.getDeclaringClass();
		try {
			MethodHandles.Lookup lookup = MethodHandles.privateLookupIn(owner, MethodHandles.lookup());
			CallSite site = LambdaMetafactory.metafactory(
				lookup,
				"handle",
				MethodType.methodType(Listener.class, owner),
				HANDLE,
				lookup.unreflect(method),
				MethodType.methodType(void.class, parameter)
			);
			return new ListenerMethod(parameter.asSubclass(Event.class), handler.priority(), handler.receiveCancelled(), site.getTarget());
		} catch (IllegalAccessException | LambdaConversionException failure) {
			throw new IllegalArgumentException("failed to bind @EventHandler " + name, failure);
		}
	}

	record ListenerMethod(Class<? extends Event> eventType, Priority priority, boolean receiveCancelled, MethodHandle factory) {
		@SuppressWarnings("unchecked")
		Listener<Event> bind(Object subscriber) {
			try {
				return (Listener<Event>) this.factory.invoke(subscriber);
			} catch (RuntimeException | Error failure) {
				throw failure;
			} catch (Throwable failure) {
				throw new IllegalStateException("failed to bind listener of " + subscriber.getClass().getName(), failure);
			}
		}
	}
}
