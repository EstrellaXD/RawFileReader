import pandas as pd

neutron = 1.0086



def extract_eic(mz: float, tolerance: float, data: pd.DataFrame) -> pd.DataFrame:
    """
    Extracts the extracted ion chromatogram (EIC) for a given m/z value and tolerance from a data frame.
    :param mz: The m/z value to extract the EIC for.
    :param tolerance: The tolerance to use for the extraction, unit is ppm.
    :param data: The data frame to extract the EIC from.
    :return: The extracted EIC.
    """
    tolerance = mz * tolerance / 1e6
    return data.loc[(data["Mass"] >= mz - tolerance) & (data["Mass"] <= mz + tolerance)]

def extract_m_p_x(mz: float, tolerance: float, x: int, data: pd.DataFrame) -> pd.DataFrame:
    """
    Extracts the extracted ion chromatogram (EIC) for a given m/z value for its M+X and tolerance from a data frame.
    :param mz: The m/z value to extract the EIC for.
    :param tolerance: The tolerance to use for the extraction, unit is ppm.
    :param x: The x value to extract the EIC for M+X.
    :param data: The data frame to extract the EIC from.
    :return: The extracted EIC.
    """
    tolerance = mz * tolerance / 1e6
    return data[(data['Mass'] >= mz - tolerance + x * neutron) & (data['Mass'] <= mz + tolerance + x * neutron)]