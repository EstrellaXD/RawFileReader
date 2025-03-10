from RawFileExacter import RawFileReader


def get_dataframe(file_path: str):
    raw_file_reader = RawFileReader(file_path)
    return raw_file_reader.to_dataframe()