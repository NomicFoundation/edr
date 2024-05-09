contract BlobReader {
  bytes32 public blob1;
  bytes32 public blob2;
  bytes32 public blob3;
  bytes32 public blob4;
  bytes32 public blob5;
  bytes32 public blob6;

  function readBlobs() external {
    bytes32 b1;
    bytes32 b2;
    bytes32 b3;
    bytes32 b4;
    bytes32 b5;
    bytes32 b6;

    assembly {
      b1 := blobhash(0)
      b2 := blobhash(1)
      b3 := blobhash(2)
      b4 := blobhash(3)
      b5 := blobhash(4)
      b6 := blobhash(5)
    }

    blob1 = b1;
    blob2 = b2;
    blob3 = b3;
    blob4 = b4;
    blob5 = b5;
    blob6 = b6;
  }
}
