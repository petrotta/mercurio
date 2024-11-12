package io.github.petrotta.mercurio.testing;

import groovy.lang.GroovyClassLoader;
import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;

import java.io.File;
import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

import static io.github.petrotta.mercurio.Application.console;

public class TestingUtils {

    public static void runTestsInDir(File testDir, Project project) throws IOException, InstantiationException, IllegalAccessException, InvocationTargetException {
        for(File test : testDir.listFiles()) {
            if(test.getName().endsWith(".groovy")) {
                GroovyClassLoader gcl = new GroovyClassLoader();

                Application.console("Running " + test.getName());

                Class theParsedClass = gcl.parseClass(test);
                Object instance = theParsedClass.newInstance();
                for(Method method : theParsedClass.getMethods()) {
                    if(method.getAnnotation(SysMLTest.class)!=null) {
                        console("Running " + method.getName());
                        method.invoke(instance, project);
                    }
                }
            }
        }

    }
}
