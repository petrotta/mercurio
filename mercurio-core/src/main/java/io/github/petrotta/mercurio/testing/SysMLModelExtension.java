package io.github.petrotta.mercurio.testing;

import io.github.petrotta.mercurio.build.Project;
import org.junit.jupiter.api.extension.*;

public class SysMLModelExtension implements  ParameterResolver {



    @Override
    public boolean supportsParameter(ParameterContext parameterContext, ExtensionContext extensionContext) throws ParameterResolutionException {
        return parameterContext.isAnnotated(SysMLModel.class) && parameterContext.getParameter().getType()== Project.class;
    }

    @Override
    public Object resolveParameter(ParameterContext parameterContext, ExtensionContext extensionContext) throws ParameterResolutionException {
        return null;
    }


}
