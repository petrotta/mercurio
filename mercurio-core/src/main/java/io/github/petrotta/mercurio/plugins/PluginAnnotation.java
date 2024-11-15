package io.github.petrotta.mercurio.plugins;



import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

@Target(ElementType.TYPE)
@Retention(RetentionPolicy.RUNTIME)
public @interface PluginAnnotation {
    String organization();
    String name();
    String version() default "1.0.0";
}