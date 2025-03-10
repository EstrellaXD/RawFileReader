import pandas as pd

from MSAnalyzer.file import get_dataframe
from MSAnalyzer.eic import extract_eic, extract_m_p_x
from pathlib import Path


formyl = 472.1586

tolerance = 5

methyl = 458.1794
folate = 440.1324

path = "Data/exp022_250306_FF_Folates_nc1_TH_3_015.raw"

data = get_dataframe(path)

file_name = Path(path).name

data.to_csv(f"Data/{file_name}.csv")

# data = pd.read_csv(f"Data/exp022_250306_FF_Folates_nc1_TH_3_015.raw.csv")
# formyl_data = extract_eic(formyl, tolerance, data).drop(["Scan", "MS Order", "Polarity"], axis=1)
# methyl_data = extract_eic(methyl, tolerance, data).drop(["Scan", "MS Order", "Polarity"], axis=1)
# folate_data = extract_eic(folate, tolerance, data).drop(["Scan", "MS Order", "Polarity"], axis=1)
# methene_data = extract_m_p_x(folate, tolerance, 1, data).drop(["Scan", "MS Order", "Polarity"], axis=1)
#
#
# formyl_data.to_csv("formyl_data.csv")
# methyl_data.to_csv("methyl_data.csv")
# folate_data.to_csv("folate_data.csv")
# methene_data.to_csv("methene_data.csv")
#
# print("Data extracted and saved to csv files")