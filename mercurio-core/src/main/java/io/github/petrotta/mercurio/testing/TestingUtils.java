package io.github.petrotta.mercurio.testing;

import groovy.junit5.plugin.JUnit5Runner;
import groovy.lang.GroovyClassLoader;
import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;
import junit.framework.TestResult;
import org.apache.groovy.test.ScriptTestAdapter;
import org.junit.platform.engine.DiscoverySelector;
import org.junit.platform.launcher.Launcher;
import org.junit.platform.launcher.LauncherDiscoveryRequest;
import org.junit.platform.launcher.core.LauncherDiscoveryRequestBuilder;
import org.junit.platform.launcher.core.LauncherFactory;
import org.junit.platform.launcher.listeners.SummaryGeneratingListener;
import org.junit.platform.launcher.listeners.TestExecutionSummary;
import org.junit.runner.JUnitCore;

import java.io.File;
import java.io.IOException;
import java.io.PrintWriter;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

import static io.github.petrotta.mercurio.Application.console;
import static org.junit.platform.engine.discovery.DiscoverySelectors.selectClass;
import static org.junit.platform.engine.discovery.DiscoverySelectors.selectFile;

public class TestingUtils {


    public static void runTestsInDir(File testDir, Project project) throws IOException, InstantiationException, IllegalAccessException, InvocationTargetException {


        GroovyClassLoader gcl = new GroovyClassLoader();

        // parse all groovy classes
        for(File test : testDir.listFiles()) {
            if (test.getName().endsWith(".groovy")) {
                Class theParsedClass = gcl.parseClass(test);
            }
        }


        for(Class loadedClass : gcl.getLoadedClasses()) {

            LauncherDiscoveryRequest request = LauncherDiscoveryRequestBuilder.request()
                    .selectors(selectClass(loadedClass))
                    .build();
            SummaryGeneratingListener listener = new SummaryGeneratingListener();

            Launcher launcher = LauncherFactory.create();
            launcher.registerTestExecutionListeners(listener);
            launcher.execute(request);


            TestExecutionSummary summary = listener.getSummary();
            if(summary.getTestsFoundCount()>0) {
                summary.printTo(new PrintWriter(System.out));
                summary.printFailuresTo(new PrintWriter(System.out));
            }

        }

    }
}
