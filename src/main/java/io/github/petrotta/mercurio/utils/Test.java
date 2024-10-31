package io.github.petrotta.mercurio.utils;

import io.github.petrotta.mercurio.Application;
import picocli.CommandLine;

import java.io.File;

public class Test {
    private static final String PILOT_SAMPLES = "C:/dev/git/SysML-v2-Pilot-Implementation/sysml/src";



    public void runTests() {

        runTest( Application.createCommandLine() , new String[] {"version"});
        runTest( Application.createCommandLine() , new String[] {"validate", "-dir", "tmp/test1"});
        runTest( Application.createCommandLine() , new String[] {"validate", "-dir", PILOT_SAMPLES+ "/examples/Mass Roll-up Example"});
        runTest( Application.createCommandLine() , new String[] {"validate", "-dir", PILOT_SAMPLES});

        runTest( Application.createCommandLine() , new String[] {"inspect", "-dir", "tmp/test1"});

        runTest( Application.createCommandLine() , new String[] {"inspect", "-dir", PILOT_SAMPLES});
        runTest( Application.createCommandLine() , new String[] {"inspect", "-dir", PILOT_SAMPLES+ "/examples/Mass Roll-up Example"});

        runTest( Application.createCommandLine() , new String[] {"eval", "-dir", PILOT_SAMPLES+ "/examples/Mass Roll-up Example", "-i", "A", "-t", "MassConstraintExample.Engine"});


        File newProject = new File("tmp/test"+System.currentTimeMillis());
        runTest( Application.createCommandLine() , new String[] {"create", "-dir", newProject.getAbsolutePath() });
        runTest( Application.createCommandLine() , new String[] {"validate", "-dir", newProject.getAbsolutePath() });


        runTest( Application.createCommandLine() , new String[] {"run", "-dir", newProject.getAbsolutePath() });







    }



    private void runTest(CommandLine commandLine, String[] args) {

        System.out.println("******************************************************");
        System.out.println(commandLine.getClass().getSimpleName() + " Output: ");
        int exitCode = commandLine.execute(args);
        System.out.println("******************************************************");

        assert (exitCode == 0);
    }

}
