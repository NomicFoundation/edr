// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";
import {Vm} from "forge-std/src/Vm.sol";

// Tests for the file system permissions cheatcodes.
contract FsPermissionsTest is Test {
    function assertEntry(
        Vm.DirEntry memory entry,
        uint64 depth,
        bool dir
    ) private pure {
        assertEq(entry.errorMessage, "");
        assertEq(entry.depth, depth);
        assertEq(entry.isDir, dir);
        assertEq(entry.isSymlink, false);
    }

    function testReadFile() public {
        string memory path = "fixtures/File/read.txt";

        assertEq(
            vm.readFile(path),
            "hello readable world\nthis is the second line!"
        );

        vm.expectRevert();
        vm.writeFile(path, "malicious stuff");

        vm.expectRevert();
        vm.readFile("/etc/hosts");

        vm.expectRevert();
        vm.readFileBinary("/etc/hosts");
    }

    function testWriteFile() public {
        string memory path = "fixtures/File/write_file.txt";
        string memory data = "hello writable world";
        vm.writeFile(path, data);

        assertEq(vm.readFile(path), data);

        vm.removeFile(path);

        vm.expectRevert();
        vm.writeFile("/etc/hosts", "malicious stuff");
        vm.expectRevert();
        vm.writeFileBinary("/etc/hosts", "malicious stuff");
    }

    function testReadDir() public {
        string memory path = "fixtures/Dir";

        {
            Vm.DirEntry[] memory entries = vm.readDir(path);
            assertEq(entries.length, 2);
            assertEntry(entries[0], 1, false);
            assertEntry(entries[1], 1, true);

            Vm.DirEntry[] memory entries2 = vm.readDir(path, 1);
            assertEq(entries2.length, 2);
            assertEq(entries[0].path, entries2[0].path);
            assertEq(entries[1].path, entries2[1].path);

            string memory contents = vm.readFile(entries[0].path);
            assertEq(contents, unicode"Wow! 😀");
        }

        {
            Vm.DirEntry[] memory entries = vm.readDir(path, 2);
            assertEq(entries.length, 4);
            assertEntry(entries[2], 2, false);
            assertEntry(entries[3], 2, true);
        }

        {
            Vm.DirEntry[] memory entries = vm.readDir(path, 3);
            assertEq(entries.length, 5);
            assertEntry(entries[4], 3, false);
        }

        vm.expectRevert();
        vm.readDir("/etc");
    }
}

contract FsNotAllowedPermissionsTest is Test {
    function testWriteLineHardhatCli() public {
        string memory root = vm.projectRoot();
        string memory clijs = string.concat(
            root,
            "/",
            "node_modules/hardhat/dist/src/cli.js"
        );

        vm.expectRevert();
        vm.writeLine(clijs, "\nffi = true\n");
    }

    function testWriteFileHardhatCli() public {
        string memory root = vm.projectRoot();
        string memory clijs = string.concat(
            root,
            "/",
            "node_modules/hardhat/dist/src/cli.js"
        );

        vm.expectRevert();
        vm.writeFile(clijs, "\nffi = true\n");
    }
}
