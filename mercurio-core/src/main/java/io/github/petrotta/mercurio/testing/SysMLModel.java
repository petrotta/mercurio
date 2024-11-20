package io.github.petrotta.mercurio.testing;

import org.junit.jupiter.api.extension.ExtendWith;

import java.lang.annotation.*;

@Target({ ElementType.FIELD, ElementType.PARAMETER })
@Retention(RetentionPolicy.RUNTIME)
@ExtendWith(SysMLModelExtension.class)
public @interface SysMLModel {
}