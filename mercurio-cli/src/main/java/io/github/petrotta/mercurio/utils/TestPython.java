package io.github.petrotta.mercurio.utils;

import org.python.util.PythonInterpreter;

import java.io.PrintWriter;

public class TestPython {

    public static void main(String[] args) {

        try(PythonInterpreter pyInterp = new PythonInterpreter()) {
            pyInterp.set("sys.path", System.getProperty("user.dir"));
            pyInterp.exec("print('Hello Python World!')");
        }
    }
}
